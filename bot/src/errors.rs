use std::error::Error;
use std::fmt::{Display, Formatter, Result};

use ytd_rs::error::YoutubeDLError;

use crate::video::DownloadedVideo;

#[derive(Debug)]
pub enum MieError {
    VideoDownloadFailed(DownloadedVideo),
    YtDlError(YoutubeDLError),
}

impl Error for MieError {}

impl Display for MieError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            MieError::VideoDownloadFailed(video) => {
                write!(f, "failed to download video: {}", video.og_url)
            }
            MieError::YtDlError(ytdl_erro) => {
                // todo: make this better not sure how
                write!(f, "ytdl error: {}", ytdl_erro.to_string())
            }
        }
    }
}
