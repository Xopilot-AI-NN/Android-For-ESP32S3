//! Просмотр Google Calendar через общий Gateway; контроллер событий единого приложения «Календарь».
use crate::apps::remote::{Controller as Remote, Endpoint};
pub struct Calendar;
impl Endpoint for Calendar {
    const KIND: u8 = 1;
    const INPUT: bool = false;
}
pub type Controller = Remote<Calendar>;
