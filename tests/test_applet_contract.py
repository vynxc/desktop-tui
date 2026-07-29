import unittest
import xml.etree.ElementTree as ElementTree
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CONFIG_SCHEMA = PROJECT_ROOT / "applet/contents/config/main.xml"
MAIN_QML = PROJECT_ROOT / "applet/contents/ui/main.qml"
SETTINGS_QML = PROJECT_ROOT / "applet/contents/ui/configGeneral.qml"

COMMAND_SETTINGS = {
    "canvasSource",
    "commandProgram",
    "commandArguments",
    "commandWorkingDirectory",
    "commandEnvironment",
    "commandExitBehavior",
    "commandIntervalSeconds",
    "commandTimeoutSeconds",
    "commandClearBetweenRuns",
}


class AppletCommandCanvasContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.main_qml = MAIN_QML.read_text()
        cls.settings_qml = SETTINGS_QML.read_text()
        root = ElementTree.parse(CONFIG_SCHEMA).getroot()
        cls.schema_keys = {
            entry.attrib["name"]
            for entry in root.findall(".//{*}entry")
        }

    def test_every_command_setting_is_declared(self):
        self.assertTrue(COMMAND_SETTINGS <= self.schema_keys)

    def test_every_command_setting_is_exposed_in_the_ui(self):
        for key in COMMAND_SETTINGS:
            with self.subTest(key=key):
                self.assertIn(f"cfg_{key}", self.settings_qml)

    def test_every_command_setting_restarts_its_instance(self):
        for key in COMMAND_SETTINGS:
            with self.subTest(key=key):
                self.assertIn(
                    f"Plasmoid.configuration.{key}",
                    self.main_qml,
                )

    def test_command_mode_uses_the_installed_supervisor_without_a_shell(self):
        self.assertIn('renderer,\n            "command"', self.main_qml)
        self.assertIn('shellProgram: "/usr/bin/env"', self.main_qml)
        for forbidden in ("/bin/sh", "/bin/bash", '"-c"', "eval("):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, self.main_qml)

    def test_shared_frames_are_renderer_only(self):
        self.assertIn("sharedFrameMode: !root.commandCanvas", self.main_qml)


if __name__ == "__main__":
    unittest.main()
