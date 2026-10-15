#!/usr/bin/env python3
"""Fast launcher lifecycle regression using isolated fake CLI/viewer processes."""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest


class DemoLauncher(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="bestiary-launch-", dir="/tmp")
        self.root = Path(self.temp.name).resolve()
        (self.root / "scripts").mkdir()
        (self.root / "target/release").mkdir(parents=True)
        shutil.copyfile(Path(__file__).with_name("run-demo"), self.root / "scripts/run-demo")
        self.executable("scripts/cargo", "#!/bin/sh\nexit 0\n")
        self.executable("target/release/bestiary", f"#!{sys.executable}\n" + r'''
import os, pathlib, signal, socket, sys, time
args = sys.argv[1:]
if args[:2] == ['world', 'new']:
    pathlib.Path(args[args.index('--out') + 1]).touch()
elif args[:2] == ['world', 'attach']:
    print(pathlib.Path(args[2]).with_suffix('.sock'))
elif args[:2] == ['world', 'run']:
    world = pathlib.Path(args[2])
    if os.environ.get('FAIL_DAEMON'):
        sys.exit(7)
    def stop(*_):
        world.with_suffix('.saved').touch()
        sys.exit(0)
    signal.signal(signal.SIGINT, stop)
    server = socket.socket(socket.AF_UNIX)
    server.bind(str(world.with_suffix('.sock')))
    server.listen()
    while True:
        time.sleep(.01)
''')
        self.executable("target/release/bestiary-viewer", f"#!{sys.executable}\n" + r'''
import json, os, pathlib, sys, time
pathlib.Path(sys.argv[1]).with_suffix('.args').write_text(json.dumps(sys.argv[1:]))
if os.environ.get('WAIT_VIEWER'):
    time.sleep(30)
sys.exit(9 if os.environ.get('FAIL_VIEWER') else 0)
''')

    def executable(self, name, contents):
        path = self.root / name
        path.write_text(contents)
        path.chmod(0o755)

    def run_demo(self, **extra):
        return subprocess.run(["sh", str(self.root / "scripts/run-demo"), "--headless", "--smoke", "image with spaces.png"], env={**os.environ, **extra}, text=True, capture_output=True, timeout=15)

    def tearDown(self):
        self.temp.cleanup()

    def test_unique_worlds_argument_passthrough_and_clean_save(self):
        for _ in range(2):
            result = self.run_demo()
            self.assertEqual(result.returncode, 0, result.stderr)
        worlds = list(self.root.glob("worlds/demo.*/world.db"))
        self.assertEqual(len(worlds), 2)
        for world in worlds:
            self.assertTrue(world.with_suffix('.saved').exists())
            self.assertEqual(json.loads(world.with_suffix('.args').read_text()), [str(world), "--headless", "--smoke", "image with spaces.png"])

    def test_viewer_failure_still_saves_daemon(self):
        result = self.run_demo(FAIL_VIEWER="1")
        self.assertEqual(result.returncode, 9, result.stderr)
        self.assertEqual(len(list(self.root.glob("worlds/demo.*/world.saved"))), 1)

    def test_daemon_failure_does_not_launch_viewer(self):
        result = self.run_demo(FAIL_DAEMON="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(list(self.root.glob("worlds/demo.*/world.args")), [])

    def test_termination_stops_viewer_and_saves_daemon(self):
        child = subprocess.Popen(["sh", str(self.root / "scripts/run-demo")], env={**os.environ, "WAIT_VIEWER": "1"}, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            deadline = time.monotonic() + 10
            while not list(self.root.glob("worlds/demo.*/world.args")):
                self.assertIsNone(child.poll())
                self.assertLess(time.monotonic(), deadline)
                time.sleep(.02)
            child.send_signal(signal.SIGTERM)
            child.communicate(timeout=10)
            self.assertEqual(child.returncode, 143)
            self.assertEqual(len(list(self.root.glob("worlds/demo.*/world.saved"))), 1)
        finally:
            if child.poll() is None:
                child.kill()
            child.communicate()


if __name__ == "__main__":
    unittest.main()
