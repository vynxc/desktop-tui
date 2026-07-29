import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami

KCM.SimpleKCM {
    id: page

    property string cfg_templateId: plasmoid.configuration.templateId
    property alias cfg_customTemplatePath: customTemplatePath.text
    property alias cfg_modelPath: modelPath.text
    property alias cfg_framesPerSecond: framesPerSecond.value
    property alias cfg_showFps: showFps.checked
    property alias cfg_animateModel: animateModel.checked
    property alias cfg_allowTextSelection: allowTextSelection.checked
    property alias cfg_fontFamily: fontFamily.text
    property alias cfg_fontPointSize: fontPointSize.value
    property alias cfg_lineSpacing: lineSpacing.value

    readonly property var templateChoices: [
        { label: i18n("Model and system information"), value: "model-system" },
        { label: i18n("Model with system sidebar"), value: "model-sidebar" },
        { label: i18n("Model only"), value: "model-only" },
        { label: i18n("System information"), value: "system" },
        { label: i18n("Compact system information"), value: "system-compact" },
        { label: i18n("Custom template file"), value: "custom" }
    ]

    Kirigami.FormLayout {
        Layout.fillWidth: true
        Layout.alignment: Qt.AlignTop

        QQC2.ComboBox {
            id: template
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
            visible: page.cfg_templateId === "custom"
            enabled: visible
            placeholderText: i18n("/path/to/template.json")
        }

        QQC2.TextField {
            id: modelPath
            Kirigami.FormData.label: i18n("Model override:")
            placeholderText: i18n("Use the template model")
        }

        Kirigami.Separator {
            Kirigami.FormData.isSection: true
            Kirigami.FormData.label: i18n("Rendering")
        }

        QQC2.SpinBox {
            id: framesPerSecond
            Kirigami.FormData.label: i18n("Frame rate (FPS):")
            from: 1
            to: 60
            editable: true
        }

        QQC2.CheckBox {
            id: showFps
            text: i18n("Show FPS counter")
        }

        QQC2.CheckBox {
            id: animateModel
            text: i18n("Animate models")
        }

        QQC2.Label {
            Layout.fillWidth: true
            text: i18n("When model animation is disabled, the renderer drops to one frame per second.")
            wrapMode: Text.WordWrap
            opacity: 0.72
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
            text: i18n("Settings belong to this widget instance, so every monitor can use a different template.")
            wrapMode: Text.WordWrap
            opacity: 0.72
        }
    }
}
