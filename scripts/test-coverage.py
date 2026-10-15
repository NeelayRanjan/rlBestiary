#!/usr/bin/env python3
"""Regression checks for production-only coverage accounting."""
import importlib.machinery
import importlib.util
from pathlib import Path
import tempfile
import unittest

loader = importlib.machinery.SourceFileLoader("bestiary_coverage", str(Path(__file__).with_name("coverage")))
spec = importlib.util.spec_from_loader(loader.name, loader)
coverage = importlib.util.module_from_spec(spec)
loader.exec_module(coverage)


class CoverageAccounting(unittest.TestCase):
    def test_test_module_filter_preserves_production_after_nested_literals(self):
        source = '''fn before() { println!("}"); }
#[cfg(test)]
mod tests {
    // } a comment cannot close the module
    fn nested() { let source = r###"{ } \" }"###; }
    /* } /* another } */ } */
    fn chars() { let c = '}'; let other = '\\u{7B}'; }
}
fn after<'a>(value: &'a str) -> &'a str { value }
'''
        ranges, external = coverage.test_modules(source)
        self.assertEqual(ranges, [(2, 8)])
        self.assertEqual(external, [])
        self.assertEqual(coverage.masked_rust(source).count("\n"), source.count("\n"))

    def test_unrecognized_test_only_item_fails_instead_of_inflating_coverage(self):
        with self.assertRaises(ValueError):
            coverage.test_modules("#[cfg(test)] fn helper() {}")
        with self.assertRaises(ValueError):
            coverage.test_modules("#[cfg(test)] mod tests {")
        with self.assertRaises(ValueError):
            coverage.test_modules("#[cfg(all(test, unix))] mod tests {}")
        with self.assertRaises(ValueError):
            coverage.test_modules('fn f() { let text = "unterminated; }')

    def test_only_production_llvm_lines_count_and_external_tests_are_excluded(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            records = []
            for crate in coverage.CRATES:
                src = root / "crates" / crate / "src"
                src.mkdir(parents=True)
                main = src / "lib.rs"
                main.write_text("fn used() {}\nfn missed() {}\n#[cfg(test)]\nmod tests;\n#[cfg(test)]\nmod inline { fn covered_test() {} }\nfn after() {}\n")
                helper = src / "tests.rs"
                helper.write_text("fn many_test_lines() {}\n")
                records.append(f"SF:{main}\nDA:1,10\nDA:2,0\nDA:6,99\nDA:7,1\nend_of_record\nSF:{helper}\nDA:1,1000\nend_of_record")
            result, excluded = coverage.production_report(root, "\n".join(records))
            self.assertEqual(len(excluded), 3)
            for summary in result.values():
                self.assertEqual(summary["covered"], 2)
                self.assertEqual(summary["executable"], 3)
                self.assertFalse(summary["pass"])
                self.assertAlmostEqual(summary["percent"], 200 / 3)
                detail = next(iter(summary["files"].values()))
                self.assertEqual(detail["uncovered_lines"], [2])
                self.assertEqual(detail["excluded_test_executable_lines"], 1)

    def test_unclassified_test_function_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            src = root / "crates/bestiary-core/src"
            src.mkdir(parents=True)
            (src / "lib.rs").write_text("#[test] fn accidental_inflation() {}")
            with self.assertRaises(ValueError):
                coverage.source_inventory(root)

    def test_coverage_floor_eighty_percent_for_core_sim_world_is_enforced_individually(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            records = []
            for crate, covered in zip(coverage.CRATES, (9, 8, 7)):
                src = root / "crates" / crate / "src"
                src.mkdir(parents=True)
                source = src / "lib.rs"
                source.write_text("\n".join(f"fn path{line}() {{}}" for line in range(1, 11)))
                lines = "\n".join(f"DA:{line},{int(line <= covered)}" for line in range(1, 11))
                records.append(f"SF:{source}\n{lines}\nend_of_record")
            result, _ = coverage.production_report(root, "\n".join(records))
            self.assertEqual(coverage.FLOOR, 80.0)
            self.assertEqual([result[crate]["pass"] for crate in coverage.CRATES], [True, True, False])
            # Aggregate coverage is 80%, but world alone fails its binding floor.
            self.assertEqual(sum(item["covered"] for item in result.values()), 24)

    def test_missing_crate_measurement_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(ValueError):
                coverage.production_report(Path(directory), "")


if __name__ == "__main__":
    unittest.main()
