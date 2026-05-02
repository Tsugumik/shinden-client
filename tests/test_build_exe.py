import unittest
import json
import tempfile
from pathlib import Path

from scripts import build_exe


class BuildExePlanTests(unittest.TestCase):
    def test_single_file_launcher_runs_preflight_bootstrap_and_build(self):
        launcher = Path(__file__).resolve().parents[1] / "generator_exe.bat"

        contents = launcher.read_text(encoding="utf-8")

        self.assertIn("setlocal EnableExtensions EnableDelayedExpansion", contents)
        self.assertIn("scripts\\build_exe.py --preflight", contents)
        self.assertIn("scripts\\build_exe.py --bootstrap --yes", contents)
        self.assertIn("scripts\\build_exe.py %BUILD_ARGS%", contents)
        self.assertIn("exit /b !ERRORLEVEL!", contents)
        self.assertIn("BOOTSTRAP_UNAVAILABLE", contents)
        self.assertIn(":refresh_path", contents)
        self.assertIn(":log_tool_lookup", contents)
        self.assertIn(":has_py3", contents)
        self.assertIn(":has_python", contents)
        self.assertIn("Preflight exit code:", contents)
        self.assertIn('start "" "%ROOT%dist-exe"', contents)

    def test_launcher_prefers_valid_py_launcher_over_windowsapps_python_alias(self):
        launcher = Path(__file__).resolve().parents[1] / "generator_exe.bat"

        contents = launcher.read_text(encoding="utf-8")

        self.assertIn("py -3 --version >nul 2>nul", contents)
        self.assertIn("python --version >nul 2>nul", contents)
        self.assertLess(contents.index("call :has_py3"), contents.index("call :has_python"))
        self.assertLess(contents.index("py -3 %*"), contents.index("python %*"))

    def test_launcher_does_not_force_winget_bootstrap_from_missing_stamp(self):
        launcher = Path(__file__).resolve().parents[1] / "generator_exe.bat"

        contents = launcher.read_text(encoding="utf-8")

        self.assertIn('if not "%PREFLIGHT_EXIT%"=="0" set "NEED_BOOTSTRAP=1"', contents)
        self.assertNotIn("BOOTSTRAP_STAMP", contents)
        self.assertNotIn(".generator-exe-bootstrap-ok", contents)

    def test_bootstrap_wrapper_can_force_bootstrap_explicitly(self):
        launcher = Path(__file__).resolve().parents[1] / "bootstrap-exe.bat"

        contents = launcher.read_text(encoding="utf-8")

        self.assertIn("call generator_exe.bat --force-bootstrap %*", contents)

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

    def test_plan_passes_local_tauri_config_to_build(self):
        root = Path("C:/project")
        config_path = root / "logs" / "tauri-local-build.conf.json"

        steps = build_exe.plan_commands(
            root,
            node_modules_exists=True,
            skip_install=False,
            tauri_config=config_path,
        )

        self.assertEqual(
            steps,
            [
                [
                    "npm",
                    "run",
                    "tauri",
                    "--",
                    "build",
                    "--config",
                    str(config_path),
                ],
            ],
        )

    def test_local_tauri_config_disables_updater_artifacts(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)

            config_path = build_exe.write_local_tauri_config(root)

            self.assertEqual(config_path, root / "logs" / "tauri-local-build.conf.json")
            contents = json.loads(config_path.read_text(encoding="utf-8"))
            self.assertEqual(contents["bundle"]["createUpdaterArtifacts"], False)

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

    def test_preflight_accepts_windows_command_shims(self):
        paths = {
            "npm.cmd": "C:/Program Files/nodejs/npm.cmd",
            "cargo.exe": "C:/Users/Kompilator/.cargo/bin/cargo.exe",
        }

        result = build_exe.preflight(tool_lookup=paths.get)

        self.assertTrue(result.ok)
        self.assertEqual(result.found_paths["npm"], paths["npm.cmd"])
        self.assertEqual(result.found_paths["cargo"], paths["cargo.exe"])

    def test_resolve_tool_accepts_windows_command_shim(self):
        paths = {"npm.cmd": "C:/Program Files/nodejs/npm.cmd"}

        self.assertEqual(
            build_exe.resolve_tool("npm", tool_lookup=paths.get),
            paths["npm.cmd"],
        )

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
