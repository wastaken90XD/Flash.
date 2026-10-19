//! Extractor for the SWF payload appended to legacy Macromedia/Adobe Windows projectors.

/// Errors returned while locating or decoding the embedded SWF payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectorError {
    /// The trailer's declared movie extent was invalid or outside the input.
    InvalidMovieExtent,
    /// No plausible SWF signature was found in the PE overlay.
    MissingMovie,
}

/// Extracts the SWF bytes appended to a Windows Flash Projector executable.
///
/// Supports the common eight-byte `0xFA123456` footer and bounded signature scanning for older
/// projector layouts. This extracts the movie only: it does not execute Windows binaries or
/// provide Wine compatibility.
pub fn extract_projector_swf(bytes: &[u8]) -> Result<&[u8], ProjectorError> {
    if bytes.len() >= 8 {
        let footer = bytes.len() - 8;
        let magic = u32::from_le_bytes([
            bytes.get(footer).copied().unwrap_or(0),
            bytes.get(footer + 1).copied().unwrap_or(0),
            bytes.get(footer + 2).copied().unwrap_or(0),
            bytes.get(footer + 3).copied().unwrap_or(0),
        ]);
        if magic == 0xFA12_3456 {
            let len = u32::from_le_bytes([
                bytes.get(footer + 4).copied().unwrap_or(0),
                bytes.get(footer + 5).copied().unwrap_or(0),
                bytes.get(footer + 6).copied().unwrap_or(0),
                bytes.get(footer + 7).copied().unwrap_or(0),
            ]) as usize;
            let start = footer
                .checked_sub(len)
                .ok_or(ProjectorError::InvalidMovieExtent)?;
            let swf = bytes
                .get(start..footer)
                .ok_or(ProjectorError::InvalidMovieExtent)?;
            return if is_swf_signature(swf) {
                Ok(swf)
            } else {
                Err(ProjectorError::InvalidMovieExtent)
            };
        }
    }

    for start in 0..bytes.len().saturating_sub(7) {
        let Some(candidate) = bytes.get(start..) else {
            continue;
        };
        if !is_swf_signature(candidate) || candidate.len() < 8 {
            continue;
        }
        let declared = u32::from_le_bytes([
            candidate.get(4).copied().unwrap_or(0),
            candidate.get(5).copied().unwrap_or(0),
            candidate.get(6).copied().unwrap_or(0),
            candidate.get(7).copied().unwrap_or(0),
        ]) as usize;
        if declared < 8 || declared > candidate.len() {
            continue;
        }
        let end = start.saturating_add(declared);
        if let Some(movie) = bytes.get(start..end) {
            if swfr_swf::Movie::parse(movie).is_ok() {
                return Ok(movie);
            }
        }
    }
    Err(ProjectorError::MissingMovie)
}

fn is_swf_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"FWS") || bytes.starts_with(b"CWS") || bytes.starts_with(b"ZWS")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use swfr_swf::Tag;
    use swfr_testkit::SwfBuilder;

    fn sample_swf() -> Vec<u8> {
        let mut b = SwfBuilder::new(8, 32, 24, 12.0, 1);
        b.push_tag(&Tag::ShowFrame);
        b.build()
    }

    #[test]
    fn extracts_projector_payload_using_fa123456_footer() {
        let swf = sample_swf();
        let mut exe = b"MZPE-stub".to_vec();
        exe.extend_from_slice(&swf);
        exe.extend_from_slice(&0xFA12_3456_u32.to_le_bytes());
        exe.extend_from_slice(&(swf.len() as u32).to_le_bytes());
        assert_eq!(extract_projector_swf(&exe), Ok(swf.as_slice()));
    }

    #[test]
    fn finds_swf_in_overlay_without_footer() {
        let swf = sample_swf();
        let mut exe = b"MZPE-stub".to_vec();
        exe.extend_from_slice(&swf);
        assert_eq!(extract_projector_swf(&exe), Ok(swf.as_slice()));
    }

    #[test]
    fn rejects_bad_footer_extent_and_unrecognised_files() {
        let mut exe = b"MZPE-stub".to_vec();
        exe.extend_from_slice(&0xFA12_3456_u32.to_le_bytes());
        exe.extend_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            extract_projector_swf(&exe),
            Err(ProjectorError::InvalidMovieExtent)
        );
        assert_eq!(
            extract_projector_swf(b"MZ/not-a-projector"),
            Err(ProjectorError::MissingMovie)
        );
    }
}
