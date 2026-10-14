use mmp_types::consts::MAX_FRAME_PAYLOAD;
use mmp_types::err::WireError;
use mmp_types::frame::{write_envelope, Frame, Summary, SummaryBucket, FT_BUNDLE};
use mmp_types::wire::BufExt;

#[test]
fn try_put_var_is_transactional_at_u16_limit() {
    let mut out = vec![0xaa];
    let before = out.clone();
    let too_large = vec![0u8; u16::MAX as usize + 1];

    assert_eq!(out.try_put_var(&too_large), Err(WireError::Oversize));
    assert_eq!(out, before);
}

#[test]
fn try_frame_encoder_rejects_wrapped_summary_count() {
    let frame = Frame::Summary(Summary {
        receipt_buckets: (0..=u8::MAX)
            .map(|bucket_epoch| SummaryBucket {
                bucket_epoch: bucket_epoch as u16,
                bloom: Vec::new(),
            })
            .collect(),
        ..Summary::default()
    });

    assert_eq!(frame.try_encode_payload(), Err(WireError::Oversize));
}

#[test]
fn try_frame_encoder_rejects_payload_over_wire_cap() {
    let frame = Frame::Bundle(vec![0u8; MAX_FRAME_PAYLOAD + 1]);
    assert_eq!(frame.try_encode(), Err(WireError::Oversize));
}

#[test]
fn try_envelope_writer_rejects_wrapping_length() {
    let payload = vec![0u8; u16::MAX as usize + 1];
    assert_eq!(
        mmp_types::frame::try_write_envelope(FT_BUNDLE, &payload),
        Err(WireError::Oversize)
    );

    // The compatibility wrapper remains byte-identical for valid payloads.
    assert_eq!(
        write_envelope(FT_BUNDLE, b"ok"),
        vec![2, 0, FT_BUNDLE, b'o', b'k']
    );
}
