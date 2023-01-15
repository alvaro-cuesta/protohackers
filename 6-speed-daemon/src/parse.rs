use std::time::Duration;

use crate::{MessageToClient, MessageToServer};
use nom::{
    branch::alt,
    bytes::streaming::tag,
    error::{Error, ErrorKind},
    multi::{length_count, length_data},
    number::streaming::{be_u16, be_u32, u8},
    Err, IResult,
};

fn parse_str(input: &[u8]) -> IResult<&[u8], &str> {
    let (input, data) = length_data(u8)(input)?;
    let data =
        std::str::from_utf8(data).map_err(|_| Err::Error(Error::new(input, ErrorKind::MapRes)))?;

    Ok((input, data))
}

#[allow(unused)]
fn parse_error(input: &[u8]) -> IResult<&[u8], MessageToClient> {
    let (input, _) = tag([0x10])(input)?;
    let (input, msg) = parse_str(input)?;

    Ok((
        input,
        MessageToClient::Error {
            msg: msg.to_owned(),
        },
    ))
}

fn parse_plate(input: &[u8]) -> IResult<&[u8], MessageToServer> {
    let (input, _) = tag([0x20])(input)?;
    let (input, plate) = parse_str(input)?;
    let (input, timestamp) = be_u32(input)?;

    Ok((
        input,
        MessageToServer::Plate {
            plate: plate.to_owned(),
            timestamp,
        },
    ))
}

#[allow(unused)]
fn parse_ticket(input: &[u8]) -> IResult<&[u8], MessageToClient> {
    let (input, _) = tag([0x21])(input)?;
    let (input, plate) = parse_str(input)?;
    let (input, road) = be_u16(input)?;
    let (input, mile1) = be_u16(input)?;
    let (input, timestamp1) = be_u32(input)?;
    let (input, mile2) = be_u16(input)?;
    let (input, timestamp2) = be_u32(input)?;
    let (input, speed) = be_u16(input)?;

    Ok((
        input,
        MessageToClient::Ticket {
            plate: plate.to_owned(),
            road,
            mile1,
            timestamp1,
            mile2,
            timestamp2,
            speed,
        },
    ))
}

fn parse_want_heartbeat(input: &[u8]) -> IResult<&[u8], MessageToServer> {
    let (input, _) = tag([0x40])(input)?;
    let (input, interval) = be_u32(input)?;
    let interval = Duration::from_millis(interval as u64 * 100);

    Ok((input, MessageToServer::WantHeartbeat { interval }))
}

#[allow(unused)]
fn parse_heartbeat(input: &[u8]) -> IResult<&[u8], MessageToClient> {
    let (input, _) = tag([0x41])(input)?;
    Ok((input, MessageToClient::Heartbeat))
}

fn parse_i_am_camera(input: &[u8]) -> IResult<&[u8], MessageToServer> {
    let (input, _) = tag([0x80])(input)?;
    let (input, road) = be_u16(input)?;
    let (input, mile) = be_u16(input)?;
    let (input, limit) = be_u16(input)?;

    Ok((input, MessageToServer::IAmCamera { road, mile, limit }))
}

fn parse_i_am_dispatcher(input: &[u8]) -> IResult<&[u8], MessageToServer> {
    let (input, _) = tag([0x81])(input)?;
    let (input, roads) = length_count(u8, be_u16)(input)?;

    Ok((input, MessageToServer::IAmDispatcher { roads }))
}

#[allow(unused)]
pub fn parse_message_to_client(input: &[u8]) -> IResult<&[u8], MessageToClient> {
    let (input, message) = alt((parse_error, parse_ticket, parse_heartbeat))(input)?;
    Ok((input, message))
}

pub fn parse_message_to_server(input: &[u8]) -> IResult<&[u8], MessageToServer> {
    let (input, message) = alt((
        parse_plate,
        parse_want_heartbeat,
        parse_i_am_camera,
        parse_i_am_dispatcher,
    ))(input)?;
    Ok((input, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error() {
        let (_, message) = parse_message_to_client(&[0x10, 0x03, 0x62, 0x61, 0x64]).unwrap();
        assert_eq!(
            message,
            MessageToClient::Error {
                msg: "bad".to_owned()
            }
        );

        let (_, message) = parse_message_to_client(&[
            0x10, 0x0b, 0x69, 0x6c, 0x6c, 0x65, 0x67, 0x61, 0x6c, 0x20, 0x6d, 0x73, 0x67,
        ])
        .unwrap();
        assert_eq!(
            message,
            MessageToClient::Error {
                msg: "illegal msg".to_owned()
            }
        );
    }

    #[test]
    fn test_parse_plate() {
        let (_, message) =
            parse_message_to_server(&[0x20, 0x04, 0x55, 0x4e, 0x31, 0x58, 0x00, 0x00, 0x03, 0xe8])
                .unwrap();
        assert_eq!(
            message,
            MessageToServer::Plate {
                plate: "UN1X".to_owned(),
                timestamp: 1000
            }
        );

        let (_, message) = parse_message_to_server(&[
            0x20, 0x07, 0x52, 0x45, 0x30, 0x35, 0x42, 0x4b, 0x47, 0x00, 0x01, 0xe2, 0x40,
        ])
        .unwrap();
        assert_eq!(
            message,
            MessageToServer::Plate {
                plate: "RE05BKG".to_owned(),
                timestamp: 123456
            }
        );
    }

    #[test]
    fn test_parse_ticket() {
        let (_, message) = parse_message_to_client(&[
            0x21, 0x04, 0x55, 0x4e, 0x31, 0x58, 0x00, 0x42, 0x00, 0x64, 0x00, 0x01, 0xe2, 0x40,
            0x00, 0x6e, 0x00, 0x01, 0xe3, 0xa8, 0x27, 0x10,
        ])
        .unwrap();
        assert_eq!(
            message,
            MessageToClient::Ticket {
                plate: "UN1X".to_owned(),
                road: 66,
                mile1: 100,
                timestamp1: 123456,
                mile2: 110,
                timestamp2: 123816,
                speed: 10000,
            }
        );

        let (_, message) = parse_message_to_client(&[
            0x21, 0x07, 0x52, 0x45, 0x30, 0x35, 0x42, 0x4b, 0x47, 0x01, 0x70, 0x04, 0xd2, 0x00,
            0x0f, 0x42, 0x40, 0x04, 0xd3, 0x00, 0x0f, 0x42, 0x7c, 0x17, 0x70,
        ])
        .unwrap();
        assert_eq!(
            message,
            MessageToClient::Ticket {
                plate: "RE05BKG".to_owned(),
                road: 368,
                mile1: 1234,
                timestamp1: 1000000,
                mile2: 1235,
                timestamp2: 1000060,
                speed: 6000,
            }
        );
    }

    #[test]
    fn test_want_heartbeat() {
        let (_, message) = parse_message_to_server(&[0x40, 0x00, 0x00, 0x00, 0x0a]).unwrap();
        assert_eq!(
            message,
            MessageToServer::WantHeartbeat {
                interval: Duration::from_secs(1) // raw = 10
            }
        );

        let (_, message) = parse_message_to_server(&[0x40, 0x00, 0x00, 0x04, 0xdb]).unwrap();
        assert_eq!(
            message,
            MessageToServer::WantHeartbeat {
                interval: Duration::from_millis(124300), // raw = 1243 = 124.3s
            }
        );
    }

    #[test]
    fn test_heartbeat() {
        let (_, message) = parse_message_to_client(&[0x41]).unwrap();
        assert_eq!(message, MessageToClient::Heartbeat);
    }

    #[test]
    fn test_i_am_camera() {
        let (_, message) =
            parse_message_to_server(&[0x80, 0x00, 0x42, 0x00, 0x64, 0x00, 0x3c]).unwrap();
        assert_eq!(
            message,
            MessageToServer::IAmCamera {
                road: 66,
                mile: 100,
                limit: 60
            }
        );

        let (_, message) =
            parse_message_to_server(&[0x80, 0x01, 0x70, 0x04, 0xd2, 0x00, 0x28]).unwrap();
        assert_eq!(
            message,
            MessageToServer::IAmCamera {
                road: 368,
                mile: 1234,
                limit: 40
            }
        );
    }

    #[test]
    fn test_i_am_dispatcher() {
        let (_, message) = parse_message_to_server(&[0x81, 0x01, 0x00, 0x42]).unwrap();
        assert_eq!(message, MessageToServer::IAmDispatcher { roads: vec![66] });

        let (_, message) =
            parse_message_to_server(&[0x81, 0x03, 0x00, 0x42, 0x01, 0x70, 0x13, 0x88]).unwrap();
        assert_eq!(
            message,
            MessageToServer::IAmDispatcher {
                roads: vec![66, 368, 5000]
            }
        );
    }
}
