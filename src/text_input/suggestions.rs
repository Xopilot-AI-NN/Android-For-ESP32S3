// Модуль словарных подсказок RU/EN.
//! Small offline prefix dictionary. Password editors never request suggestions.
const RU: &[&str] = &[
    "Минск",
    "Москва",
    "Могилёв",
    "Молодечно",
    "Брест",
    "Борисов",
    "Барановичи",
    "Гомель",
    "Гродно",
    "Витебск",
    "Вильнюс",
    "Варшава",
    "Киев",
    "Казань",
    "Калининград",
    "Санкт-Петербург",
    "Сочи",
    "Самара",
    "Лондон",
    "Париж",
    "Берлин",
    "Алматы",
    "Астана",
    "Тбилиси",
    "Ереван",
    "Денис",
    "Александр",
    "Алексей",
    "Андрей",
    "Анна",
    "Мария",
    "мои",
    "мне",
    "можно",
    "привет",
    "пока",
    "пожалуйста",
    "спасибо",
    "сегодня",
    "сейчас",
    "хорошо",
    "часы",
    "устройство",
    "телефон",
    "дом",
    "работа",
];
/// Параметр EN компонента словарных подсказок RU/EN.
const EN: &[&str] = &[
    "AOSP",
    "Android",
    "Denis",
    "Watch",
    "Minsk",
    "Moscow",
    "London",
    "Paris",
    "Berlin",
    "Warsaw",
    "Vilnius",
    "New",
    "York",
    "San",
    "Francisco",
    "Tokyo",
    "hello",
    "thanks",
    "please",
    "today",
    "tomorrow",
    "home",
    "work",
    "phone",
    "my",
    "watch",
    "yes",
    "no",
];
/// Находит начало последнего слова по границам UTF-8.
fn word_start(text: &str) -> usize {
    text.char_indices()
        .rev()
        .find(|(_, c)| !c.is_alphabetic() && *c != '-')
        .map_or(0, |(i, c)| i + c.len_utf8())
}
/// Подбирает до трёх слов по префиксу и выбранному языку.
pub fn candidates(text: &str, english: bool) -> [&'static str; 3] {
    let prefix = &text[word_start(text)..];
    let mut result = [""; 3];
    if prefix.is_empty() {
        return result;
    }
    let mut count = 0;
    for &word in if english { EN } else { RU } {
        if word.chars().count() > prefix.chars().count()
            && prefix
                .chars()
                .zip(word.chars())
                .all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
        {
            result[count] = word;
            count += 1;
            if count == result.len() {
                break;
            }
        }
    }
    result
}
/// Replace the current word atomically; an overlong completion keeps the draft intact.
pub fn complete<const N: usize>(text: &mut heapless::String<N>, word: &str) -> bool {
    let start = word_start(text);
    let mut next = text.clone();
    next.truncate(start);
    if next.push_str(word).is_err() || next.push(' ').is_err() {
        return false;
    }
    *text = next;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Проверяет дополнение кириллического слова и сохранность ввода при переполнении.
    fn completion_handles_cyrillic_current_word_and_capacity_atomically() {
        assert_eq!(candidates("город ми", false)[0], "Минск");
        assert_eq!(candidates("HE", true)[0], "hello");
        let mut draft = heapless::String::<32>::try_from("город ми").unwrap();
        assert!(complete(&mut draft, "Минск"));
        assert_eq!(draft.as_str(), "город Минск ");
        let mut short = heapless::String::<4>::try_from("ми").unwrap();
        assert!(!complete(&mut short, "Минск"));
        assert_eq!(short.as_str(), "ми");
    }
    #[test]
    /// Проверяет возврат фокуса на видимую клавишу после исчезновения подсказок.
    fn accepting_last_suggestion_keeps_the_crown_on_a_visible_key() {
        let mut keyboard = super::super::Keyboard::default();
        keyboard.suggest("ми");
        keyboard.view.focus = (keyboard.view.character_count() + 7) as u8;
        assert_eq!(keyboard.press(), super::super::Action::Suggestion("Минск"));
        keyboard.suggest("Минск ");
        assert!(keyboard.view.selected_action(super::super::SPACE));
        keyboard.rotate(i32::MAX);
        keyboard.rotate(i32::MIN);
        assert!(usize::from(keyboard.view.focus) < keyboard.view.character_count() + 7);
    }
}
