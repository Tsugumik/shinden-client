import unittest
from pathlib import Path

from scripts import build_exe


class BuildExePlanTests(unittest.TestCase):
    def test_plan_skips_install_when_node_modules_exist(self):
        root = Path("C:/project")

        steps = build_exe.plan_commands(root, node_modules_exists=True, skip_install=False)

        self.assertEqual(steps, [["npm", "run", "tauri", "--", "build"]])

    def test_plan_installs_dependencies_when_node_modules_are_missing(self):
        root = Path("C:/project")

        steps = build_exe.plan_commands(root, node_modules_exists=False, skip_install=False)

        self.assertEqual(
            steps,
            [
                ["npm", "install"],
                ["npm", "run", "tauri", "--", "build"],
            ],
        )

    def test_project_environment_points_logs_at_project_root(self):
        root = Path("C:/project")

        env = build_exe.build_environment(root, base_env={"PATH": "example"})

        self.assertEqual(env["SHINDEN_CLIENT_LOG_DIR"], str(root / "logs"))
        self.assertEqual(env["SHINDEN_BUILD_PROJECT_ROOT"], str(root))
        self.assertEqual(env["PATH"], "example")


if __name__ == "__main__":
    unittest.main()
