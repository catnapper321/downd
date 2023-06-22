use crate::DownloaderCommand;
use nom::{
    branch::alt,
    bytes::complete::tag_no_case,
    character::complete::{digit1, not_line_ending, space1},
    combinator::{map, map_res},
    sequence::separated_pair,
    Finish, IResult,
};
use std::str::FromStr;

fn parse_int<T: FromStr>(input: &str) -> IResult<&str, T> {
    map_res(digit1, |x| T::from_str(x))(input)
}

fn add_url_cmd(input: &str) -> IResult<&str, DownloaderCommand> {
    let p = separated_pair(tag_no_case("add"), space1, not_line_ending);
    map(p, |(_, url): (_, &str)| {
        DownloaderCommand::AddUrl(url.to_string())
    })(input)
}

fn pause_cmd(input: &str) -> IResult<&str, DownloaderCommand> {
    let p = tag_no_case("pause");
    map(p, |_| DownloaderCommand::Pause)(input)
}

fn cancel_cmd(input: &str) -> IResult<&str, DownloaderCommand> {
    let p = tag_no_case("cancel");
    map(p, |_| DownloaderCommand::Cancel)(input)
}

fn resume_cmd(input: &str) -> IResult<&str, DownloaderCommand> {
    let p = tag_no_case("resume");
    map(p, |_| DownloaderCommand::Resume)(input)
}

fn movedown_cmd(input: &str) -> IResult<&str, DownloaderCommand> {
    let p = separated_pair(tag_no_case("down"), space1, parse_int);
    map(p, |(_, index)| DownloaderCommand::MoveDown(index))(input)
}

fn moveup_cmd(input: &str) -> IResult<&str, DownloaderCommand> {
    let p = separated_pair(tag_no_case("up"), space1, parse_int);
    map(p, |(_, index)| DownloaderCommand::MoveUp(index))(input)
}

fn delete_cmd(input: &str) -> IResult<&str, DownloaderCommand> {
    let p = separated_pair(tag_no_case("delete"), space1, parse_int);
    map(p, |(_, index)| DownloaderCommand::Delete(index))(input)
}

impl FromStr for DownloaderCommand {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut cmds = alt((
            add_url_cmd,
            pause_cmd,
            cancel_cmd,
            resume_cmd,
            movedown_cmd,
            moveup_cmd,
            delete_cmd,
        ));
        if let Ok((_, cmd)) = cmds(s).finish() {
            Ok(cmd)
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
mod checks {
    use super::*;
    #[test]
    fn check_addurl() {
        let input = "add www.google.com\n";
        let cmd = DownloaderCommand::AddUrl("www.google.com".into());
        assert_eq!(input.parse(), Ok(cmd));
    }
    #[test]
    fn check_pause() {
        let input = "Pause\n";
        let cmd = DownloaderCommand::Pause;
        assert_eq!(input.parse(), Ok(cmd));
    }
    #[test]
    fn check_cancel() {
        let input = "cancel\n";
        let cmd = DownloaderCommand::Cancel;
        assert_eq!(input.parse(), Ok(cmd));
    }
    #[test]
    fn check_resume() {
        let input = "RESUME\n";
        let cmd = DownloaderCommand::Resume;
        assert_eq!(input.parse(), Ok(cmd));
    }
    #[test]
    fn check_uint_parse() {
        let input = "1234";
        assert_eq!(parse_int::<usize>(input), Ok(("", 1234)));
        let input = "1234\n";
        assert_eq!(parse_int::<usize>(input), Ok(("\n", 1234)));
    }
    #[test]
    fn check_movedown() {
        let input = "down 4\n";
        let cmd = DownloaderCommand::MoveDown(4);
        assert_eq!(input.parse(), Ok(cmd));
    }
    #[test]
    fn check_moveup() {
        let input = "Up 2\n";
        let cmd = DownloaderCommand::MoveUp(2);
        assert_eq!(input.parse(), Ok(cmd));
    }
    #[test]
    fn check_delete() {
        let input = "DeLeTe 2\n";
        let cmd = DownloaderCommand::Delete(2);
        // assert_eq!(delete_cmd(input), Ok(("\n", cmd)));
        assert_eq!(input.parse(), Ok(cmd));
    }
}
