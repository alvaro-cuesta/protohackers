use super::LrcpMessage;
use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, is_not, tag},
    character::complete::digit1,
    combinator::{eof, value},
    IResult,
};

fn parse_u32_digits(input: &[u8]) -> IResult<&[u8], u32> {
    let (input, session) = digit1(input)?;

    let session = std::str::from_utf8(session).unwrap();
    let session = session.parse::<u32>().map_err(|_| {
        nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::TooLarge,
        ))
    })?;

    Ok((input, session))
}

fn parse_connect(input: &[u8]) -> IResult<&[u8], LrcpMessage> {
    let (input, _) = tag("connect/")(input)?;
    let (input, session) = parse_u32_digits(input)?;
    let (input, _) = tag("/")(input)?;
    eof(input)?;

    Ok((b"", LrcpMessage::Connect { session }))
}

fn parse_data(input: &[u8]) -> IResult<&[u8], LrcpMessage> {
    let (input, _) = tag("data/")(input)?;
    let (input, session) = parse_u32_digits(input)?;
    let (input, _) = tag("/")(input)?;
    let (input, position) = parse_u32_digits(input)?;
    let (input, _) = tag("/")(input)?;
    let (input, data) = escaped_transform(
        is_not(r#"\/"#),
        '\\',
        alt((
            value(b"\\".as_slice(), tag(b"\\")),
            value(b"/".as_slice(), tag(b"/")),
        )),
    )(input)?;
    let (input, _) = nom::bytes::complete::tag("/")(input)?;
    eof(input)?;

    Ok((
        b"",
        LrcpMessage::Data {
            session,
            position,
            data: data.to_vec(),
        },
    ))
}

fn parse_ack(input: &[u8]) -> IResult<&[u8], LrcpMessage> {
    let (input, _) = nom::bytes::complete::tag("ack/")(input)?;
    let (input, session) = parse_u32_digits(input)?;
    let (input, _) = nom::bytes::complete::tag("/")(input)?;
    let (input, length) = parse_u32_digits(input)?;
    let (input, _) = nom::bytes::complete::tag("/")(input)?;
    nom::combinator::eof(input)?;

    Ok((b"", LrcpMessage::Ack { session, length }))
}

fn parse_close(input: &[u8]) -> IResult<&[u8], LrcpMessage> {
    let (input, _) = nom::bytes::complete::tag("close/")(input)?;
    let (input, session) = parse_u32_digits(input)?;
    let (input, _) = nom::bytes::complete::tag("/")(input)?;
    nom::combinator::eof(input)?;

    Ok((b"", LrcpMessage::Close { session }))
}

pub(super) fn parse_message(input: &[u8]) -> IResult<&[u8], LrcpMessage> {
    let (input, _) = nom::bytes::complete::tag("/")(input)?;

    nom::branch::alt((parse_connect, parse_data, parse_ack, parse_close))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_data() {
        assert!(parse_data(b"hello").is_err());

        assert_eq!(
            parse_data(b"data/1234/5678/hello/"),
            Ok((
                b"".as_slice(),
                LrcpMessage::Data {
                    session: 1234,
                    position: 5678,
                    data: b"hello".to_vec()
                }
            ))
        );

        assert_eq!(
            parse_data(br#"data/1234/5678/123\\456/"#),
            Ok((
                b"".as_slice(),
                LrcpMessage::Data {
                    session: 1234,
                    position: 5678,
                    data: br#"123\456"#.to_vec()
                }
            ))
        );
    }
}
