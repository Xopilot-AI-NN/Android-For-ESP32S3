//! Короткие распространённые фразы доступны offline; остальное требует API Gateway.
use crate::apps::remote::{Controller as Remote, Endpoint};
pub struct Translator;
impl Endpoint for Translator {
    const KIND: u8 = 3;
    const INPUT: bool = true;
    fn offline(text: &str) -> Option<&'static str> {
        match text.trim().to_lowercase().as_str() {
            "hello" => Some("Привет"),
            "привет" => Some("Hello"),
            "hello world" => Some("Привет, мир"),
            "good morning" => Some("Доброе утро"),
            "доброе утро" => Some("Good morning"),
            "thank you" | "thanks" => Some("Спасибо"),
            "спасибо" => Some("Thank you"),
            "please" => Some("Пожалуйста"),
            "пожалуйста" => Some("Please"),
            "yes" => Some("Да"),
            "да" => Some("Yes"),
            "no" => Some("Нет"),
            "нет" => Some("No"),
            "goodbye" => Some("До свидания"),
            "до свидания" => Some("Goodbye"),
            "water" => Some("Вода"),
            "вода" => Some("Water"),
            "help" => Some("Помогите"),
            "помогите" => Some("Help"),
            "how are you" => Some("Как дела?"),
            "как дела" => Some("How are you?"),
            _ => None,
        }
    }
}
pub type Controller = Remote<Translator>;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dictionary_is_explicit_and_does_not_fake_cloud_translation() {
        assert_eq!(Translator::offline("hello"), Some("Привет"));
        assert_eq!(Translator::offline("an unknown phrase"), None);
    }
}
