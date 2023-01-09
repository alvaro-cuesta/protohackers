use crate::Price;

#[derive(Debug)]
pub struct Response(Price);

impl Response {
    pub fn new(price: Price) -> Self {
        Self(price)
    }
}

impl From<Response> for [u8; 4] {
    fn from(value: Response) -> Self {
        value.0.to_be_bytes()
    }
}
