use super::*;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1, not_line_ending},
    combinator::{map, map_res},
    sequence::terminated,
    Finish, IResult,
};

fn digit_field(input: &str) -> IResult<&str, Option<u64>> {
    let na = map(tag("NA|"), |_| None);
    let p = map(terminated(digit1, char('|')), |x: &str| {
        x.parse::<u64>().ok()
    });
    let mut p2 = alt((p, na));
    p2(input)
}

fn parse_download_line(input: &str) -> IResult<&str, DownloaderMsg> {
    let (i, _) = tag("DOWNLOAD|")(input)?;
    let (i, downloaded_bytes) = map_res(digit1, |x: &str| x.parse::<u64>())(i)?;
    let (i, _) = tag("|")(i)?;
    let (i, total_bytes) = digit_field(i)?;
    let (i, frag_index) = digit_field(i)?;
    let (i, frag_count) = digit_field(i)?;
    Ok((
        i,
        DownloaderMsg::Downloading {
            downloaded_bytes,
            total_bytes,
            frag_index,
            frag_count,
        },
    ))
}

fn parse_moved_line(input: &str) -> IResult<&str, DownloaderMsg> {
    let (i, _) = tag("MOVED|")(input)?;
    let (i, title) = map(not_line_ending, String::from)(i)?;
    let title = if title == "NA" { None } else { Some(title) };
    Ok((i, DownloaderMsg::Moved(title)))
}

fn parse_title_line(input: &str) -> IResult<&str, DownloaderMsg> {
    let (i, _) = tag("START|")(input)?;
    let (i, title) = map(not_line_ending, String::from)(i)?;
    let title = if title == "NA" { None } else { Some(title) };
    Ok((i, DownloaderMsg::Starting(title)))
}

pub fn parse_progress_update_line(line: &str) -> Result<DownloaderMsg, nom::error::Error<&str>> {
    let mut p = alt((parse_download_line, parse_moved_line, parse_title_line));
    p(line).finish().map(|x| x.1)
}
