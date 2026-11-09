//! Read-only People API contacts; Google credentials remain in Gateway.
use crate::apps::remote::{Controller as Remote, Endpoint};
pub struct Contacts;
impl Endpoint for Contacts {
    const KIND: u8 = 4;
    const INPUT: bool = false;
}
pub type Controller = Remote<Contacts>;
