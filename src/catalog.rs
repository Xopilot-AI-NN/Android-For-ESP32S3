// Каталог нативных компонентов; проверяется по manifest.json при упаковке образа.
/// Описание компонента приложения, доступное runtime без динамического выделения памяти.
pub struct Descriptor { pub id: u8, pub package: &'static str, pub label: &'static str, pub screen: Option<u8> }
/// Все стабильные ID, включая разделы настроек и мировое время внутри часов.
pub const COMPONENTS: [Descriptor; 23] = [
    Descriptor { id: 0, package: "com.android.deskclock", label: "Часы", screen: Some(6) },
    Descriptor { id: 1, package: "com.android.wear.media", label: "Медиа", screen: Some(10) },
    Descriptor { id: 2, package: "com.android.settings", label: "Настройки", screen: Some(5) },
    Descriptor { id: 3, package: "com.android.settings", label: "Связь", screen: Some(7) },
    Descriptor { id: 4, package: "com.android.settings", label: "Об устройстве", screen: Some(8) },
    Descriptor { id: 5, package: "com.android.wear.assistant", label: "Ассистент", screen: Some(27) },
    Descriptor { id: 6, package: "com.xopilot.calculator", label: "Калькулятор", screen: None },
    Descriptor { id: 7, package: "com.xopilot.files", label: "Файлы", screen: None },
    Descriptor { id: 8, package: "com.xopilot.share", label: "Передача файлов", screen: None },
    Descriptor { id: 9, package: "com.android.settings", label: "Диагностика", screen: None },
    Descriptor { id: 10, package: "com.xopilot.flashlight", label: "Фонарик", screen: Some(29) },
    Descriptor { id: 11, package: "com.android.settings", label: "Память и swap", screen: None },
    Descriptor { id: 12, package: "com.xopilot.weather", label: "Погода", screen: None },
    Descriptor { id: 13, package: "com.android.deskclock", label: "Мировое время", screen: None },
    Descriptor { id: 14, package: "com.xopilot.breathing", label: "Дыхание", screen: None },
    Descriptor { id: 15, package: "com.xopilot.calendar", label: "Календарь", screen: None },
    Descriptor { id: 16, package: "com.xopilot.converter", label: "Конвертер", screen: None },
    Descriptor { id: 17, package: "com.xopilot.counter", label: "Счётчик", screen: None },
    Descriptor { id: 18, package: "com.xopilot.editor", label: "Текстовый редактор", screen: None },
    Descriptor { id: 19, package: "com.xopilot.tasks", label: "Задачи", screen: None },
    Descriptor { id: 20, package: "com.xopilot.expenses", label: "Расходы", screen: None },
    Descriptor { id: 21, package: "com.xopilot.focus", label: "Фокус", screen: None },
    Descriptor { id: 22, package: "com.xopilot.phone", label: "Телефон", screen: None },
];
/// Приложения меню в порядке отображения.
pub const LAUNCHER: [u8; 18] = [0, 1, 2, 5, 6, 7, 8, 10, 12, 14, 15, 16, 17, 18, 19, 20, 21, 22];
/// Установленные пакеты, включая системные поверхности.
pub const PACKAGES: &str = "com.android.deskclock com.android.wear.media com.android.settings com.android.wear.assistant com.xopilot.calculator com.xopilot.files com.xopilot.share com.xopilot.flashlight com.xopilot.weather com.xopilot.breathing com.xopilot.calendar com.xopilot.converter com.xopilot.counter com.xopilot.editor com.xopilot.tasks com.xopilot.expenses com.xopilot.focus com.xopilot.phone com.android.wear.launcher com.android.wear.widgets com.android.systemui";
/// Формирует таблицу подписей по стабильным ID.
const fn labels() -> [&'static str; 23] { let mut result = [""; 23]; let mut i = 0; while i < 23 { result[i] = COMPONENTS[i].label; i += 1; } result }
pub const LABELS: [&str; 23] = labels();
