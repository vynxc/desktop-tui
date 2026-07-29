import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami

KCM.SimpleKCM {
    id: page

    property string cfg_canvasSource: plasmoid.configuration.canvasSource
    property string cfg_templateId: plasmoid.configuration.templateId
    property alias cfg_customTemplatePath: customTemplatePath.text
    property alias cfg_modelPath: modelPath.text
    property alias cfg_framesPerSecond: framesPerSecond.value
    property alias cfg_showFps: showFps.checked
    property alias cfg_animateModel: animateModel.checked
    property alias cfg_commandProgram: commandProgram.text
    property alias cfg_commandArguments: commandArguments.text
    property alias cfg_commandWorkingDirectory: commandWorkingDirectory.text
    property alias cfg_commandEnvironment: commandEnvironment.text
    property string cfg_commandExitBehavior: plasmoid.configuration.commandExitBehavior
    property alias cfg_commandIntervalSeconds: commandIntervalSeconds.value
    property alias cfg_commandTimeoutSeconds: commandTimeoutSeconds.value
    property alias cfg_commandClearBetweenRuns: commandClearBetweenRuns.checked
    property alias cfg_allowTextSelection: allowTextSelection.checked
    property alias cfg_fontFamily: fontFamily.text
    property alias cfg_fontPointSize: fontPointSize.value
    property alias cfg_lineSpacing: lineSpacing.value

    readonly property bool rendererCanvas: cfg_canvasSource !== "command"
    readonly property var canvasChoices: [
        { label: i18n("Desktop TUI renderer"), value: "renderer" },
        { label: i18n("Command output"), value: "command" }
    ]
    readonly property var templateChoices: [
        { label: i18n("Model and system information"), value: "model-system" },
        { label: i18n("Model with system sidebar"), value: "model-sidebar" },
        { label: i18n("Model only"), value: "model-only" },
        { label: i18n("System information"), value: "system" },
        { label: i18n("Compact system information"), value: "system-compact" },
        { label: i18n("Custom template file"), value: "custom" }
    ]
    readonly property var exitBehaviorChoices: [
        { label: i18n("Run once and keep output"), value: "keep-output" },
        { label: i18n("Run on an interval"), value: "interval" },
        { label: i18n("Keep running"), value: "restart" }
    ]

    Kirigami.FormLayout {
        Layout.fillWidth: true
        Layout.alignment: Qt.AlignTop

        QQC2.ComboBox {
            id: canvasSource
            Kirigami.FormData.label: i18n("Canvas source:")
            model: page.canvasChoices
            textRole: "label"
            valueRole: "value"
            Component.onCompleted: currentIndex = indexOfValue(page.cfg_canvasSource || "renderer")
            onActivated: page.cfg_canvasSource = currentValue
        }

        Kirigami.Separator {
            visible: page.rendererCanvas
            Kirigami.FormData.isSection: true
            Kirigami.FormData.label: i18n("Renderer")
        }

        QQC2.ComboBox {
            id: template
            visible: page.rendererCanvas
            enabled: visible
            Kirigami.FormData.label: i18n("Template:")
            model: page.templateChoices
            textRole: "label"
            valueRole: "value"
            Component.onCompleted: currentIndex = indexOfValue(page.cfg_templateId)
            onActivated: page.cfg_templateId = currentValue
        }

        QQC2.TextField {
            id: customTemplatePath
            Kirigami.FormData.label: i18n("Template file:")
            visible: page.rendererCanvas && page.cfg_templateId === "custom"
            enabled: visible
            placeholderText: i18n("/path/to/template.json")
        }

        QQC2.TextField {
            id: modelPath
            visible: page.rendererCanvas
            enabled: visible
            Kirigami.FormData.label: i18n("Model override:")
            placeholderText: i18n("Use the template model")
        }

        Kirigami.Separator {
            visible: page.rendererCanvas
            Kirigami.FormData.isSection: true
            Kirigami.FormData.label: i18n("Rendering")
        }

        QQC2.SpinBox {
            id: framesPerSecond
            visible: page.rendererCanvas
            enabled: visible
            Kirigami.FormData.label: i18n("Frame rate (FPS):")
            from: 1
            to: 60
            editable: true
        }

        QQC2.CheckBox {
            id: showFps
            visible: page.rendererCanvas
            enabled: visible
            text: i18n("Show FPS counter")
        }

        QQC2.CheckBox {
            id: animateModel
            visible: page.rendererCanvas
            enabled: visible
            text: i18n("Animate models")
        }

        QQC2.Label {
            visible: page.rendererCanvas
            Layout.fillWidth: true
            text: i18n("When model animation is disabled, the renderer drops to one frame per second.")
            wrapMode: Text.WordWrap
            opacity: 0.72
        }

        Kirigami.Separator {
            visible: !page.rendererCanvas
            Kirigami.FormData.isSection: true
            Kirigami.FormData.label: i18n("Command")
        }

        QQC2.TextField {
            id: commandProgram
            visible: !page.rendererCanvas
            enabled: visible
            Kirigami.FormData.label: i18n("Program:")
            placeholderText: i18n("fastfetch")
        }

        QQC2.TextArea {
            id: commandArguments
            visible: !page.rendererCanvas
            enabled: visible
            Kirigami.FormData.label: i18n("Arguments:")
            Layout.fillWidth: true
            Layout.preferredHeight: 88
            placeholderText: i18n("One argument per line")
            wrapMode: TextEdit.NoWrap
        }

        QQC2.TextField {
            id: commandWorkingDirectory
            visible: !page.rendererCanvas
            enabled: visible
            Kirigami.FormData.label: i18n("Working directory:")
            placeholderText: i18n("Inherit the widget directory")
        }

        QQC2.TextArea {
            id: commandEnvironment
            visible: !page.rendererCanvas
            enabled: visible
            Kirigami.FormData.label: i18n("Environment:")
            Layout.fillWidth: true
            Layout.preferredHeight: 88
            placeholderText: i18n("NAME=value, one per line")
            wrapMode: TextEdit.NoWrap
        }

        QQC2.Label {
            visible: !page.rendererCanvas
            Layout.fillWidth: true
            text: i18n("The program is launched directly without a shell. Arguments are passed exactly as written.")
            wrapMode: Text.WordWrap
            opacity: 0.72
        }

        QQC2.ComboBox {
            id: commandExitBehavior
            visible: !page.rendererCanvas
            enabled: visible
            Kirigami.FormData.label: i18n("After exit:")
            model: page.exitBehaviorChoices
            textRole: "label"
            valueRole: "value"
            Component.onCompleted: currentIndex = indexOfValue(page.cfg_commandExitBehavior || "keep-output")
            onActivated: page.cfg_commandExitBehavior = currentValue
        }

        QQC2.SpinBox {
            id: commandIntervalSeconds
            visible: !page.rendererCanvas && page.cfg_commandExitBehavior === "interval"
            enabled: visible
            Kirigami.FormData.label: i18n("Interval (seconds):")
            from: 1
            to: 86400
            editable: true
        }

        QQC2.SpinBox {
            id: commandTimeoutSeconds
            visible: !page.rendererCanvas
            enabled: visible
            Kirigami.FormData.label: i18n("Timeout (seconds):")
            from: 0
            to: 86400
            editable: true
            textFromValue: function(value) {
                return value === 0 ? i18n("Disabled") : String(value);
            }
        }

        QQC2.CheckBox {
            id: commandClearBetweenRuns
            visible: !page.rendererCanvas && page.cfg_commandExitBehavior !== "keep-output"
            enabled: visible
            text: i18n("Clear the canvas between runs")
        }

        Kirigami.Separator {
            Kirigami.FormData.isSection: true
            Kirigami.FormData.label: i18n("Terminal")
        }

        QQC2.TextField {
            id: fontFamily
            Kirigami.FormData.label: i18n("Font family:")
        }

        QQC2.SpinBox {
            id: fontPointSize
            Kirigami.FormData.label: i18n("Font size:")
            from: 6
            to: 32
        }

        QQC2.SpinBox {
            id: lineSpacing
            Kirigami.FormData.label: i18n("Line spacing:")
            from: 0
            to: 8
        }

        QQC2.CheckBox {
            id: allowTextSelection
            text: i18n("Allow terminal text selection")
        }

        QQC2.Label {
            Kirigami.FormData.isSection: true
            Layout.fillWidth: true
            text: i18n("Settings belong to this widget instance, so every monitor can use a different canvas.")
            wrapMode: Text.WordWrap
            opacity: 0.72
        }
    }
}
