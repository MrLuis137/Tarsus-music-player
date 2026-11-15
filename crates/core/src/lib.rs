pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

pub mod decoder;
pub use decoder::{AudioDecoder, DecodeError};
pub mod output;
pub use output::{AudioOutput,OutputError};
pub mod converter;
pub use converter::Converter;
