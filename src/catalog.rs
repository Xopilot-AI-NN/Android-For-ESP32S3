// Stable component IDs are kept even when an app is removed; saved state cannot alias a new app.
pub struct Descriptor {
    pub id: u8,
    pub package: &'static str,
    pub label: &'static str,
    pub screen: Option<u8>,
}
pub const COMPONENTS: [Descriptor; 32] = [
    Descriptor {
        id: 0,
        package: "com.android.deskclock",
        label: "Часы",
        screen: Some(6),
    },
    Descriptor {
        id: 1,
        package: "com.android.wear.media",
        label: "Медиа",
        screen: Some(10),
    },
    Descriptor {
        id: 2,
        package: "com.android.settings",
        label: "Настройки",
        screen: Some(5),
    },
    Descriptor {
        id: 3,
        package: "com.android.settings",
        label: "Связь",
        screen: Some(7),
    },
    Descriptor {
        id: 4,
        package: "com.android.settings",
        label: "Об устройстве",
        screen: Some(8),
    },
    Descriptor {
        id: 5,
        package: "",
        label: "",
        screen: None,
    },
    Descriptor {
        id: 6,
        package: "com.xopilot.calculator",
        label: "Калькулятор",
        screen: None,
    },
    Descriptor {
        id: 7,
        package: "com.xopilot.files",
        label: "Файлы",
        screen: None,
    },
    Descriptor {
        id: 8,
        package: "com.xopilot.share",
        label: "Передача файлов",
        screen: None,
    },
    Descriptor {
        id: 9,
        package: "com.android.settings",
        label: "Диагностика",
        screen: None,
    },
    Descriptor {
        id: 10,
        package: "com.xopilot.flashlight",
        label: "Фонарик",
        screen: Some(29),
    },
    Descriptor {
        id: 11,
        package: "com.android.settings",
        label: "Память и swap",
        screen: None,
    },
    Descriptor {
        id: 12,
        package: "com.xopilot.weather",
        label: "Погода",
        screen: None,
    },
    Descriptor {
        id: 13,
        package: "com.android.deskclock",
        label: "Мировое время",
        screen: None,
    },
    Descriptor {
        id: 14,
        package: "",
        label: "",
        screen: None,
    },
    Descriptor {
        id: 15,
        package: "com.xopilot.calendar",
        label: "Календарь",
        screen: None,
    },
    Descriptor {
        id: 16,
        package: "",
        label: "",
        screen: None,
    },
    Descriptor {
        id: 17,
        package: "",
        label: "",
        screen: None,
    },
    Descriptor {
        id: 18,
        package: "com.xopilot.editor",
        label: "Текстовый редактор",
        screen: None,
    },
    Descriptor {
        id: 19,
        package: "com.xopilot.tasks",
        label: "Задачи",
        screen: None,
    },
    Descriptor {
        id: 20,
        package: "",
        label: "",
        screen: None,
    },
    Descriptor {
        id: 21,
        package: "com.xopilot.focus",
        label: "Фокус",
        screen: None,
    },
    Descriptor {
        id: 22,
        package: "com.xopilot.phone",
        label: "Телефон",
        screen: None,
    },
    Descriptor {
        id: 23,
        package: "com.xopilot.gallery",
        label: "Галерея",
        screen: None,
    },
    Descriptor {
        id: 24,
        package: "com.xopilot.btkeyboard",
        label: "BT-клавиатура",
        screen: None,
    },
    Descriptor {
        id: 25,
        package: "com.xopilot.google",
        label: "Google",
        screen: None,
    },
    Descriptor {
        id: 26,
        package: "com.xopilot.googlecalendar",
        label: "Google Calendar",
        screen: None,
    },
    Descriptor {
        id: 27,
        package: "com.xopilot.gmail",
        label: "Gmail",
        screen: None,
    },
    Descriptor {
        id: 28,
        package: "com.xopilot.translator",
        label: "Переводчик",
        screen: None,
    },
    Descriptor {
        id: 29,
        package: "com.xopilot.contacts",
        label: "Контакты Google",
        screen: None,
    },
    Descriptor {
        id: 30,
        package: "com.xopilot.messages",
        label: "Сообщения",
        screen: None,
    },
    Descriptor {
        id: 31,
        package: "com.xopilot.browser",
        label: "Браузер",
        screen: None,
    },
];
pub const LAUNCHER: [u8; 21] = [
    0, 1, 2, 6, 7, 8, 10, 12, 15, 18, 19, 21, 22, 23, 24, 25, 27, 28, 29, 30, 31,
];
pub const PACKAGES: &str = "com.android.deskclock com.android.wear.media com.android.settings com.xopilot.calculator com.xopilot.files com.xopilot.share com.xopilot.flashlight com.xopilot.weather com.xopilot.calendar com.xopilot.editor com.xopilot.tasks com.xopilot.focus com.xopilot.phone com.xopilot.gallery com.xopilot.btkeyboard com.xopilot.google com.xopilot.googlecalendar com.xopilot.gmail com.xopilot.translator com.xopilot.contacts com.xopilot.messages com.xopilot.browser com.android.wear.launcher com.android.wear.widgets com.android.systemui";
const fn labels() -> [&'static str; 32] {
    let mut labels = [""; 32];
    let mut i = 0;
    while i < 32 {
        labels[i] = COMPONENTS[i].label;
        i += 1;
    }
    labels
}
pub const LABELS: [&str; 32] = labels();
