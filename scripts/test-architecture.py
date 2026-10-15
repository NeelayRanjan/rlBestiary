#!/usr/bin/env python3
"""Offline architecture/determinism regression gates, plus detector regressions.

Cargo supplies the resolved transitive normal/build graph for all three desktop
targets. Source checks mask comments, strings and cfg(test) modules, then reject
forbidden APIs rather than prose or legitimate neural motor/muscle-force math.
This complements behavioral determinism and genesis property tests; it is not a
formal proof about arbitrary Rust metaprogramming or dependency internals.
"""
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parent.parent
SIMULATION = ("bestiary-core", "bestiary-sim", "bestiary-world", "bestiary-archive")
TARGETS = ("aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc")
loader = importlib.machinery.SourceFileLoader("bestiary_coverage", str(ROOT / "scripts/coverage"))
spec = importlib.util.spec_from_loader(loader.name, loader)
source_tools = importlib.util.module_from_spec(spec)
loader.exec_module(source_tools)

FORBIDDEN_DEPENDENCY = re.compile(
    r"^(?:bevy(?:_|$)|bevy-|egui(?:_|$)|eframe$|winit$|wgpu(?:-|$)|"
    r"iced(?:_|$)|gtk\d?(?:-|$)|tokio(?:-|$)|async-|smol$|futures(?:-|$))"
)
DETERMINISM_APIS = re.compile(
    r"\b(?:Instant|SystemTime|thread_rng|ThreadRng|OsRng|HashMap|HashSet|"
    r"RandomState|async|await)\b|\b(?:rand\s*::\s*)?random\s*(?:::<[^;]*?>)?\s*\(|"
    r"\b(?:Utc|Local)\s*::\s*(?:now|today)\s*\("
)
# Rapier 0.23.1's Generic/Revolute/Prismatic/Spherical/Rope joint motor
# setters/builders, motor fields, and types. Neural `bone.motor.step(...)`
# and muscle add_force_at_point(...) remain legal.
JOINT_MOTOR_APIS = re.compile(
    r"\b(?:JointMotor|MotorModel)\b|\b(?:set_motor\w*|motor_model|motor_velocity|"
    r"motor_position|motor_max_force)\s*\(|\.\s*(?:motor_axes|motors)\b|"
    r"\.\s*motor\s*\("
)


def dependency_paths(metadata, root_name):
    """Return normal/build paths (dev-only tests do not run in the library)."""
    packages = {item["id"]: item["name"] for item in metadata["packages"]}
    nodes = {item["id"]: item for item in metadata["resolve"]["nodes"]}
    starts = [key for key, name in packages.items() if name == root_name]
    if len(starts) != 1:
        raise ValueError("expected exactly one package " + root_name)
    pending, seen, result = [(starts[0], [root_name])], set(), []
    while pending:
        identity, path = pending.pop(0)
        if identity in seen:
            continue
        seen.add(identity)
        if FORBIDDEN_DEPENDENCY.search(packages[identity]):
            result.append(" -> ".join(path))
        for dependency in sorted(nodes[identity]["deps"], key=lambda item: item["pkg"]):
            if any(kind["kind"] != "dev" for kind in dependency["dep_kinds"]):
                pending.append((dependency["pkg"], path + [packages[dependency["pkg"]]]))
    return result


def production_text(source):
    ranges, _ = source_tools.test_modules(source)
    lines = source_tools.masked_rust(source).splitlines(keepends=True)
    for first, last in ranges:
        for index in range(first - 1, last):
            lines[index] = "".join("\n" if char == "\n" else " " for char in lines[index])
    return "".join(lines)


def violations(source, pattern):
    masked = production_text(source)
    return [(masked.count("\n", 0, match.start()) + 1, match.group()) for match in pattern.finditer(masked)]


def source_violations(crates, pattern):
    inventory, excluded = source_tools.source_inventory(ROOT, crates)
    result = []
    for path in sorted(inventory):
        if path in excluded:
            continue
        result.extend(f"{path.relative_to(ROOT)}:{line}: {token}" for line, token in violations(path.read_text(), pattern))
    return result


class CompletionArchitecture(unittest.TestCase):
    def test_no_simulation_crate_depends_on_bevy_rendering_ui_or_async(self):
        for target in TARGETS:
            command = [str(ROOT / "scripts/cargo"), "metadata", "--offline", "--locked", "--format-version", "1", "--filter-platform", target]
            metadata = json.loads(subprocess.check_output(command, cwd=ROOT, text=True))
            for crate in SIMULATION:
                with self.subTest(target=target, crate=crate):
                    self.assertEqual(dependency_paths(metadata, crate), [])

    def test_simulation_uses_no_wallclock_thread_local_rng_or_order_sensitive_hashmap(self):
        # No production HashMap/HashSet use currently needs a cache exception.
        self.assertEqual(source_violations(SIMULATION, DETERMINISM_APIS), [])

    def test_no_code_path_anywhere_applies_a_joint_motor_torque(self):
        workspace = tuple(path.name for path in sorted((ROOT / "crates").iterdir()) if path.is_dir())
        self.assertEqual(source_violations(workspace, JOINT_MOTOR_APIS), [])


class ArchitectureDetectorRegression(unittest.TestCase):
    def test_dependency_graph_finds_transitive_normal_and_build_edges(self):
        metadata = {
            "packages": [{"id": name, "name": name} for name in ("bestiary-core", "bridge", "bevy_render", "tokio", "egui")],
            "resolve": {"nodes": [
                {"id": "bestiary-core", "deps": [
                    {"pkg": "bridge", "dep_kinds": [{"kind": None}]},
                    {"pkg": "egui", "dep_kinds": [{"kind": "dev"}]},
                ]},
                {"id": "bridge", "deps": [
                    {"pkg": "bevy_render", "dep_kinds": [{"kind": None}]},
                    {"pkg": "tokio", "dep_kinds": [{"kind": "build"}]},
                ]},
                *[{"id": name, "deps": []} for name in ("bevy_render", "tokio", "egui")],
            ]},
        }
        self.assertEqual(dependency_paths(metadata, "bestiary-core"), [
            "bestiary-core -> bridge -> bevy_render", "bestiary-core -> bridge -> tokio",
        ])
        with self.assertRaises(ValueError):
            dependency_paths(metadata, "missing-crate")

    def test_forbidden_apis_are_detected_in_executable_code(self):
        for source in (
            "let now = std::time::Instant::now();", "let now = SystemTime::now();",
            "let now = chrono::Local::now();",
            "let rng = rand::thread_rng();", "let value = rand::random::<u32>();",
            "use std::collections::HashMap as State;", "async fn rule() {}",
        ):
            with self.subTest(source=source):
                self.assertTrue(violations(source, DETERMINISM_APIS))
        for source in (
            "joint.set_motor_velocity(axis, speed, factor);", "builder.motor(0.1, 0.2, 1.0, 2.0);",
            "let motor = JointMotor::default();", "joint.motor_axes = axes;",
            "joint.motors[0].max_force = 100.;", "builder.motor_position(axis, x, k, c);",
        ):
            with self.subTest(source=source):
                self.assertTrue(violations(source, JOINT_MOTOR_APIS))

    def test_comments_literals_tests_and_contractile_muscle_math_are_not_joint_motors(self):
        source = '''// Never use Instant, HashMap, or joint.set_motor_position(...).
/* thread_rng() /* joint.motors[0] */ */
fn step() {
    let message = r##"JointMotor and SystemTime { motor_velocity() }"##;
    let signals = bone.motor.step(&sensors);
    let source = Input::Local(0);
    body.add_force_at_point(force, endpoint, true);
    let torque = lever.cross(&force);
}
#[cfg(test)]
mod tests { fn assert_unmotorized() { assert!(joint.motor_axes.is_empty()); } }
'''
        self.assertEqual(violations(source, DETERMINISM_APIS), [])
        self.assertEqual(violations(source, JOINT_MOTOR_APIS), [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
