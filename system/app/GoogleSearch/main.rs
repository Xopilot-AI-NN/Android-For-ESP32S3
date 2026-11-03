//! Поисковый запрос и управление аккаунтом; результаты предоставляет настроенный SearchProvider.
use crate::apps::remote::{Controller as Remote, Endpoint};
pub struct Search;
impl Endpoint for Search {
    const KIND: u8 = 0;
    const INPUT: bool = true;
}
pub type Controller = Remote<Search>;
