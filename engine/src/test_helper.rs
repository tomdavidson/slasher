#![allow(dead_code)]

use proptest::prelude::*;

use crate::fence::{PendingFence, accept_fence_line};

pub(crate) fn valid_command_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9\\-]{0,15}".prop_filter("no trailing hyphen", |s| !s.ends_with('-'))
}

pub(crate) fn feed_body(fence: PendingFence, lines: &[String]) -> PendingFence {
    lines.iter().enumerate().fold(fence, |f, (i, line)| {
        let (next, _) = accept_fence_line(f, i + 1, line);
        next
    })
}
