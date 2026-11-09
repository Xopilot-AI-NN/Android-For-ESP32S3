//! Read-only Gmail: интерфейс не выдумывает письма и не принимает Google-пароль на часах.
use crate::apps::remote::{Controller as Remote, Endpoint};
pub struct Mail;
impl Endpoint for Mail {
    const KIND: u8 = 2;
    const INPUT: bool = false;
}
pub type Controller = Remote<Mail>;
