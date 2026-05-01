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

    def test_preflight_reports_missing_build_tools(self):
        result = build_exe.preflight(tool_lookup=lambda name: None)

        self.assertFalse(result.ok)
        self.assertEqual(
            [tool.name for tool in result.missing_required],
            ["npm", "cargo"],
        )
        self.assertIn("Node.js", result.summary())
        self.assertIn("--bootstrap", result.summary())

    def test_preflight_passes_when_required_tools_exist(self):
        result = build_exe.preflight(tool_lookup=lambda name: f"C:/tools/{name}.exe")

        self.assertTrue(result.ok)
        self.assertEqual(result.missing_required, [])

    def test_winget_bootstrap_commands_install_missing_packages(self):
        result = build_exe.preflight(tool_lookup=lambda name: None)

        commands = build_exe.winget_install_commands(result, accept_agreements=True)

        self.assertEqual(
            commands,
            [
                [
                    "winget",
                    "install",
                    "--id",
                    "OpenJS.NodeJS.LTS",
                    "-e",
                    "--accept-source-agreements",
                    "--accept-package-agreements",
                ],
                [
                    "winget",
                    "install",
                    "--id",
                    "Rustlang.Rustup",
                    "-e",
                    "--accept-source-agreements",
                    "--accept-package-agreements",
                ],
                [
                    "winget",
                    "install",
                    "--id",
                    "Microsoft.VisualStudio.2022.BuildTools",
                    "-e",
                    "--override",
                    "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended",
                    "--accept-source-agreements",
                    "--accept-package-agreements",
                ],
                [
                    "winget",
                    "install",
                    "--id",
                    "Microsoft.EdgeWebView2Runtime",
                    "-e",
                    "--accept-source-agreements",
                    "--accept-package-agreements",
                ],
            ],
        )

    def test_winget_bootstrap_still_installs_windows_build_prerequisites(self):
        result = build_exe.preflight(tool_lookup=lambda name: f"C:/tools/{name}.exe")

        commands = build_exe.winget_install_commands(result)

        self.assertEqual(
            commands,
            [
                [
                    "winget",
                    "install",
                    "--id",
                    "Microsoft.VisualStudio.2022.BuildTools",
                    "-e",
                    "--override",
                    "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended",
                ],
                [
                    "winget",
                    "install",
                    "--id",
                    "Microsoft.EdgeWebView2Runtime",
                    "-e",
                ],
            ],
        )


if __name__ == "__main__":
    unittest.main()
