import QtCore
import QtQuick
import QtQuick.Layouts
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasmoid
import "QMLTermWidget" as Terminal

PlasmoidItem {
    id: root

    Plasmoid.backgroundHints: PlasmaCore.Types.NoBackground
    preferredRepresentation: fullRepresentation

    readonly property string installRoot: localPath(
        StandardPaths.writableLocation(StandardPaths.HomeLocation)
    ) + "/.local/libexec/desktop-tui"
    readonly property string renderer: installRoot + "/desktop-tui"
    readonly property string runtimeRoot: localPath(
        StandardPaths.writableLocation(StandardPaths.RuntimeLocation)
    )
    readonly property string instanceKey: String(Plasmoid.containment.id) + "-" + String(Plasmoid.id)
    readonly property string sharedFramePath: runtimeRoot + "/desktop-tui-" + instanceKey + ".bin"
    readonly property string rendererSignature: [
        Plasmoid.configuration.templateId || "model-system",
        Plasmoid.configuration.customTemplatePath || "",
        Plasmoid.configuration.modelPath || "",
        Plasmoid.configuration.framesPerSecond || 15,
        Plasmoid.configuration.showFps || false,
        Plasmoid.configuration.animateModel === undefined
            ? true
            : Plasmoid.configuration.animateModel,
        Plasmoid.configuration.fontFamily || "monospace",
        Plasmoid.configuration.fontPointSize || 11,
        Plasmoid.configuration.lineSpacing || 1
    ].join("\u001f")

    property bool initialized: false
    property int runningColumns: 0
    property int runningLines: 0
    property var terminalLoaderItem: null

    function localPath(location) {
        const value = location.toString();
        return value.startsWith("file://")
            ? decodeURIComponent(value.substring(7))
            : value;
    }

    function normalizedColumns(columns) {
        return Math.max(20, columns || 160);
    }

    function normalizedLines(lines) {
        return Math.max(8, lines || 48);
    }

    function rendererArguments(columns, lines) {
        const configuration = Plasmoid.configuration;
        const templateId = configuration.templateId || "model-system";
        const customTemplatePath = configuration.customTemplatePath || "";
        const args = [
            "MALLOC_ARENA_MAX=1",
            "MALLOC_TRIM_THRESHOLD_=0",
            "DESKTOP_TUI_SHARED_FRAME=" + sharedFramePath,
            "DESKTOP_TUI_FRAME_WIDTH=" + normalizedColumns(columns),
            "DESKTOP_TUI_FRAME_HEIGHT=" + normalizedLines(lines),
            "DESKTOP_TUI_TEMPLATE_DIR=" + installRoot + "/templates",
            "DESKTOP_TUI_ASSET_DIR=" + installRoot + "/assets",
            "DESKTOP_TUI_MODEL_PATH=" + (configuration.modelPath || ""),
            "DESKTOP_TUI_FPS=" + (configuration.framesPerSecond || 15),
            "DESKTOP_TUI_SHOW_FPS=" + (configuration.showFps ? "1" : "0"),
            "DESKTOP_TUI_ANIMATE_MODEL="
                + (configuration.animateModel === false ? "0" : "1")
        ];

        if (templateId === "custom" && customTemplatePath.length > 0) {
            args.push("DESKTOP_TUI_TEMPLATE_FILE=" + customTemplatePath);
        } else {
            args.push("DESKTOP_TUI_TEMPLATE=" + templateId);
        }
        args.push(renderer);
        return args;
    }

    function startRenderer(terminal, session) {
        runningColumns = normalizedColumns(terminal.columns);
        runningLines = normalizedLines(terminal.lines);
        session.shellProgramArgs = rendererArguments(runningColumns, runningLines);
        session.startShellProgram();
    }

    function restartRenderer() {
        if (terminalLoaderItem === null) {
            return;
        }
        terminalLoaderItem.active = false;
        reloadTimer.restart();
    }

    onRendererSignatureChanged: {
        if (initialized) {
            restartTimer.restart();
        }
    }

    Component.onCompleted: initialized = true

    Timer {
        id: restartTimer
        interval: 350
        repeat: false
        onTriggered: root.restartRenderer()
    }

    Timer {
        id: reloadTimer
        interval: 25
        repeat: false
        onTriggered: {
            if (root.terminalLoaderItem !== null) {
                root.terminalLoaderItem.active = true;
            }
        }
    }

    fullRepresentation: Item {
        Layout.preferredWidth: 1100
        Layout.preferredHeight: 620
        Layout.minimumWidth: 320
        Layout.minimumHeight: 180

        Loader {
            id: terminalLoader
            anchors.fill: parent
            active: true
            sourceComponent: terminalComponent
            Component.onCompleted: root.terminalLoaderItem = terminalLoader
            Component.onDestruction: {
                if (root.terminalLoaderItem === terminalLoader) {
                    root.terminalLoaderItem = null;
                }
            }
        }
    }

    Component {
        id: terminalComponent

        Terminal.QMLTermWidget {
            id: terminal

            focus: false
            font.family: Plasmoid.configuration.fontFamily || "monospace"
            font.pointSize: Plasmoid.configuration.fontPointSize || 11
            lineSpacing: Plasmoid.configuration.lineSpacing || 1
            colorScheme: "DesktopTuiTransparent"
            enableBold: true
            enableItalic: true
            antialiasText: true
            blinkingCursor: false
            useFBORendering: false
            sharedFramePath: root.sharedFramePath
            sharedFrameMode: true
            mouseInputEnabled: Plasmoid.configuration.allowTextSelection || false

            session: Terminal.QMLTermSession {
                id: rendererSession
                initialWorkingDirectory: "/"
                shellProgram: "/usr/bin/env"
            }

            Component.onCompleted: Qt.callLater(root.startRenderer, terminal, rendererSession)
        }
    }
}
