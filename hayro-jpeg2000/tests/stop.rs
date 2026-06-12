//! Tests for cooperative cancellation via [`StopCheck`].

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use hayro_jpeg2000::{DecodeError, DecodeSettings, Image, StopCheck};

const SAMPLE: &[u8] = include_bytes!("../assets/stop-test.jp2");

/// Returns a check that stops after `n` polls.
fn stop_after(n: usize) -> StopCheck {
    let remaining = Arc::new(AtomicUsize::new(n));
    StopCheck::new(move || {
        if remaining.load(Ordering::Relaxed) == 0 {
            true
        } else {
            remaining.fetch_sub(1, Ordering::Relaxed);
            false
        }
    })
}

fn decode_with(stop: StopCheck) -> Result<Vec<u8>, DecodeError> {
    let mut image = Image::new(SAMPLE, &DecodeSettings::default()).unwrap();
    image.set_stop_check(stop);
    image.decode()
}

#[test]
fn never_stopping_matches_plain_decode() {
    let plain = Image::new(SAMPLE, &DecodeSettings::default())
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(decode_with(StopCheck::none()).unwrap(), plain);
    assert_eq!(decode_with(stop_after(usize::MAX)).unwrap(), plain);
}

#[test]
fn stopping_returns_stopped() {
    for polls in [0, 1] {
        let err = decode_with(stop_after(polls)).unwrap_err();
        assert_eq!(err, DecodeError::Stopped);
    }
}
