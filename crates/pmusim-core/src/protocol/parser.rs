use crate::error::{PmuError, Result};
use super::constants::{parse_sync, FrameType, ProtocolVersion};
use super::crc16::crc16;
use super::frame::*;

fn read_u16(data: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([data[off], data[off + 1]])
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn read_i16(data: &[u8], off: usize) -> i16 {
    i16::from_be_bytes([data[off], data[off + 1]])
}

fn read_f32(data: &[u8], off: usize) -> f32 {
    f32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

/// Decode a fixed-width IDCODE field byte-preserving. Spec says IDCODE
/// is 8 ASCII characters, but field-observed substations sometimes
/// emit latin-1 / GBK bytes — `from_utf8_lossy` would replace those
/// with U+FFFD, after which `encode_ascii_padded` (in the builder)
/// would expand each replacement to 3 UTF-8 bytes and the resulting
/// 8-byte slot would no longer match the substation's IDCODE. Latin-1
/// codepoint-per-byte gives a perfect round-trip: byte N decodes to
/// Unicode U+00N and re-encodes back to byte N.
fn decode_ascii(data: &[u8]) -> String {
    let end = data.iter().rposition(|&b| b != 0).map(|i| i + 1).unwrap_or(0);
    data[..end].iter().map(|&b| b as char).collect()
}

fn decode_gbk(data: &[u8]) -> String {
    let (cow, _, _) = encoding_rs::GBK.decode(data);
    cow.trim_end_matches('\0').to_string()
}

/// Parse a single frame.
///
/// `format_flags` is the FORMAT word from the matching CFG-2 — only used
/// when decoding a Data frame; pass 0 for command/config frames or when
/// you don't have CFG-2 yet (data fields will then be decoded as int16
/// per spec default).
pub fn parse(
    data: &[u8],
    format_flags: u16,
    phnmr: u16,
    annmr: u16,
    dgnmr: u16,
) -> Result<Frame> {
    if data.len() < 4 {
        return Err(PmuError::Parse("Frame too short".into()));
    }

    let sync = read_u16(data, 0);
    let (frame_type, version) = parse_sync(sync).map_err(|e| PmuError::Parse(e))?;

    let size = read_u16(data, 2) as usize;
    // Smallest legal frame is V2 command (20 bytes); V3 command is 24.
    // A SIZE of 0/1 would underflow `size - 2` below — release builds
    // wrap to ~usize::MAX, then `data[off]` panics with an obscure
    // out-of-bounds. Reject the frame cleanly here instead.
    const MIN_FRAME_SIZE: usize = 8; // SYNC + SIZE + at least an IDCODE byte + CRC
    if size < MIN_FRAME_SIZE {
        return Err(PmuError::Parse(format!(
            "Frame SIZE field too small: {size} < {MIN_FRAME_SIZE}"
        )));
    }
    // §8 every frame must be exactly SIZE bytes — extra bytes mean the
    // caller fed us a glued buffer. Reject (was silently accepting before;
    // a future UDP/replay path would have eaten the next frame as trailing
    // garbage with no diagnostic).
    if data.len() != size {
        return Err(PmuError::Parse(format!(
            "Frame length mismatch: SIZE={size} but data.len()={}",
            data.len()
        )));
    }

    // CRC check
    let expected_crc = read_u16(data, size - 2);
    let actual_crc = crc16(&data[..size - 2]);
    if expected_crc != actual_crc {
        return Err(PmuError::CrcMismatch {
            expected: expected_crc,
            actual: actual_crc,
        });
    }

    match frame_type {
        FrameType::Command => parse_command(data, version),
        FrameType::Cfg1 | FrameType::Cfg2 => parse_config(data, version, frame_type),
        FrameType::Data => parse_data(data, version, format_flags, phnmr, annmr, dgnmr),
    }
}

fn parse_command(data: &[u8], version: ProtocolVersion) -> Result<Frame> {
    match version {
        ProtocolVersion::V2 => {
            // V2 Command (20 bytes): SYNC(2) + SIZE(2) + SOC(4) + IDCODE(8) + CMD(2) + CRC(2)
            let soc = read_u32(data, 4);
            let idcode = decode_ascii(&data[8..16]);
            let cmd = read_u16(data, 16);
            Ok(Frame::Command(CommandFrame {
                version,
                idcode,
                soc,
                fracsec: 0,
                cmd,
            }))
        }
        ProtocolVersion::V3 => {
            // V3 Command (24 bytes): SYNC(2) + SIZE(2) + IDCODE(8) + SOC(4) + FRACSEC(4) + CMD(2) + CRC(2)
            let idcode = decode_ascii(&data[4..12]);
            let soc = read_u32(data, 12);
            let fracsec = read_u32(data, 16);
            let cmd = read_u16(data, 20);
            Ok(Frame::Command(CommandFrame {
                version,
                idcode,
                soc,
                fracsec,
                cmd,
            }))
        }
    }
}

fn parse_config(data: &[u8], version: ProtocolVersion, frame_type: FrameType) -> Result<Frame> {
    let cfg_type = match frame_type {
        FrameType::Cfg1 => 2,
        FrameType::Cfg2 => 3,
        _ => unreachable!(),
    };

    let (idcode, soc, fracsec, d_frame, meas_rate, num_pmu, pmu_start) = match version {
        ProtocolVersion::V2 => {
            let soc = read_u32(data, 4);
            let d_frame = read_u16(data, 8);
            let meas_rate = read_u32(data, 10);
            let num_pmu = read_u16(data, 14);
            (String::new(), soc, 0u32, d_frame, meas_rate, num_pmu, 16usize)
        }
        ProtocolVersion::V3 => {
            let idcode = decode_ascii(&data[4..12]);
            let soc = read_u32(data, 12);
            let fracsec = read_u32(data, 16);
            let meas_rate = read_u32(data, 20);
            let num_pmu = read_u16(data, 24);
            (idcode, soc, fracsec, 0u16, meas_rate, num_pmu, 26usize)
        }
    };

    // Parse each PMU data block. Per V3 §8.2 layout repeats NUM_PMU
    // times; the per-PMU FNOM+PERIOD live at the end of each block, so
    // we must walk all blocks even if the caller only cares about #0.
    let mut off = pmu_start;
    let mut blocks: Vec<PmuBlock> = Vec::with_capacity(num_pmu as usize);
    for _ in 0..num_pmu {
        let stn = decode_gbk(&data[off..off + 16]);
        off += 16;
        let pmu_idcode = decode_ascii(&data[off..off + 8]);
        off += 8;
        let format_flags = read_u16(data, off);
        off += 2;
        let phnmr = read_u16(data, off);
        off += 2;
        let annmr = read_u16(data, off);
        off += 2;
        let dgnmr = read_u16(data, off);
        off += 2;

        let total_channels = phnmr as usize + annmr as usize + dgnmr as usize * 16;
        let mut channel_names = Vec::with_capacity(total_channels);
        for _ in 0..total_channels {
            channel_names.push(decode_gbk(&data[off..off + 16]));
            off += 16;
        }
        let mut phunit = Vec::with_capacity(phnmr as usize);
        for _ in 0..phnmr {
            phunit.push(read_u32(data, off));
            off += 4;
        }
        let mut anunit = Vec::with_capacity(annmr as usize);
        for _ in 0..annmr {
            anunit.push(read_u32(data, off));
            off += 4;
        }
        let mut digunit = Vec::with_capacity(dgnmr as usize);
        for _ in 0..dgnmr {
            let hi = read_u16(data, off);
            let lo = read_u16(data, off + 2);
            digunit.push((hi, lo));
            off += 4;
        }
        let fnom = read_u16(data, off);
        off += 2;
        let period = read_u16(data, off);
        off += 2;

        blocks.push(PmuBlock {
            stn, pmu_idcode, format_flags, phnmr, annmr, dgnmr,
            channel_names, phunit, anunit, digunit, fnom, period,
        });
    }

    // After all NUM_PMU blocks the V3 spec puts CHK only — there is no
    // top-level FNOM/PERIOD; those are per-PMU. The convenience copies
    // on ConfigFrame mirror block #0.
    let first = blocks.first().cloned().unwrap_or(PmuBlock {
        stn: String::new(), pmu_idcode: String::new(), format_flags: 0,
        phnmr: 0, annmr: 0, dgnmr: 0,
        channel_names: vec![], phunit: vec![], anunit: vec![], digunit: vec![],
        fnom: 0, period: 0,
    });

    // V2: primary idcode comes from per-PMU field; V3: from DC_IDCODE in header
    let primary_idcode = match version {
        ProtocolVersion::V2 => first.pmu_idcode.clone(),
        ProtocolVersion::V3 => idcode,
    };

    Ok(Frame::Config(ConfigFrame {
        version,
        cfg_type,
        idcode: primary_idcode,
        soc,
        fracsec,
        d_frame,
        meas_rate,
        num_pmu,
        stn: first.stn.clone(),
        pmu_idcode: first.pmu_idcode.clone(),
        format_flags: first.format_flags,
        phnmr: first.phnmr,
        annmr: first.annmr,
        dgnmr: first.dgnmr,
        channel_names: first.channel_names.clone(),
        phunit: first.phunit.clone(),
        anunit: first.anunit.clone(),
        digunit: first.digunit.clone(),
        fnom: first.fnom,
        period: first.period,
        pmu_blocks: blocks,
    }))
}

fn parse_data(
    data: &[u8],
    version: ProtocolVersion,
    format_flags: u16,
    phnmr: u16,
    annmr: u16,
    dgnmr: u16,
) -> Result<Frame> {
    let (idcode, soc, fracsec, stat, val_start) = match version {
        ProtocolVersion::V2 => {
            let soc = read_u32(data, 4);
            let fracsec = read_u32(data, 8);
            let stat = read_u16(data, 12);
            (String::new(), soc, fracsec, stat, 14usize)
        }
        ProtocolVersion::V3 => {
            let idcode = decode_ascii(&data[4..12]);
            let soc = read_u32(data, 12);
            let fracsec = read_u32(data, 16);
            let stat = read_u16(data, 20);
            (idcode, soc, fracsec, stat, 22usize)
        }
    };

    let mut off = val_start;

    let phasor_float = DataFrame::phasors_are_float(format_flags);
    let analog_float = DataFrame::analog_is_float(format_flags);
    let freq_float = DataFrame::freq_is_float(format_flags);

    let mut phasors = Vec::with_capacity(phnmr as usize);
    for _ in 0..phnmr {
        let (a, b) = if phasor_float {
            let a = read_f32(data, off) as f64;
            let b = read_f32(data, off + 4) as f64;
            off += 8;
            (a, b)
        } else {
            let a = read_i16(data, off) as f64;
            let b = read_i16(data, off + 2) as f64;
            off += 4;
            (a, b)
        };
        phasors.push((a, b));
    }

    let (freq, dfreq) = if freq_float {
        let f = read_f32(data, off) as f64;
        let df = read_f32(data, off + 4) as f64;
        off += 8;
        (f, df)
    } else {
        let f = read_i16(data, off) as f64;
        let df = read_i16(data, off + 2) as f64;
        off += 4;
        (f, df)
    };

    let mut analog = Vec::with_capacity(annmr as usize);
    for _ in 0..annmr {
        let v = if analog_float {
            let v = read_f32(data, off) as f64;
            off += 4;
            v
        } else {
            let v = read_i16(data, off) as f64;
            off += 2;
            v
        };
        analog.push(v);
    }

    let mut digital = Vec::with_capacity(dgnmr as usize);
    for _ in 0..dgnmr {
        digital.push(read_u16(data, off));
        off += 2;
    }

    Ok(Frame::Data(DataFrame {
        version,
        idcode,
        soc,
        fracsec,
        stat,
        format_flags,
        phasors,
        freq,
        dfreq,
        analog,
        digital,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::ProtocolVersion;

    #[test]
    fn v2_request_cfg1() {
        let data = hex::decode("aa4200146757dd1d30475830304750310004a5cb").unwrap();
        let frame = parse(&data, 0, 0, 0, 0).unwrap();
        if let Frame::Command(cmd) = frame {
            assert_eq!(cmd.version, ProtocolVersion::V2);
            assert_eq!(cmd.idcode, "0GX00GP1");
            assert_eq!(cmd.soc, 0x6757DD1D);
            assert_eq!(cmd.cmd, 0x0004);
        } else {
            panic!("Expected Command frame");
        }
    }

    #[test]
    fn v2_heartbeat() {
        let data = hex::decode("aa4200146757dd22304758303047503140009cf7").unwrap();
        let frame = parse(&data, 0, 0, 0, 0).unwrap();
        if let Frame::Command(cmd) = frame {
            assert_eq!(cmd.cmd, 0x4000);
        } else {
            panic!("Expected Command frame");
        }
    }

    #[test]
    fn v3_request_cfg1() {
        let data =
            hex::decode("aa430018304758303047503167b2c719000000000004ac08").unwrap();
        let frame = parse(&data, 0, 0, 0, 0).unwrap();
        if let Frame::Command(cmd) = frame {
            assert_eq!(cmd.version, ProtocolVersion::V3);
            assert_eq!(cmd.idcode, "0GX00GP1");
            assert_eq!(cmd.soc, 0x67B2C719);
            assert_eq!(cmd.fracsec, 0);
            assert_eq!(cmd.cmd, 0x0004);
        } else {
            panic!("Expected Command frame");
        }
    }

    #[test]
    fn v3_heartbeat() {
        let data =
            hex::decode("aa430018304758303047503167b2c71e000000004000f804").unwrap();
        let frame = parse(&data, 0, 0, 0, 0).unwrap();
        if let Frame::Command(cmd) = frame {
            assert_eq!(cmd.cmd, 0x4000);
        } else {
            panic!("Expected Command frame");
        }
    }

    #[test]
    fn v2_data() {
        // Correct hex from Python reference tests (44 bytes)
        let data = hex::decode(
            "aa02002c67a99d11000d9490000000000000012c0bb823d700c8000000000000000023d700000000000a21f3",
        )
        .unwrap();
        let frame = parse(&data, 0, 0, 11, 1).unwrap();
        if let Frame::Data(df) = frame {
            assert_eq!(df.version, ProtocolVersion::V2);
            assert_eq!(df.idcode, "");
            assert_eq!(df.soc, 0x67A99D11);
            assert_eq!(df.fracsec, 0x000D9490);
            assert_eq!(df.analog.len(), 11);
            assert_eq!(df.analog[0], 0x012C as f64);
            assert_eq!(df.analog[1], 0x0BB8 as f64);
            assert_eq!(df.analog[2], 0x23D7 as f64);
            assert_eq!(df.digital, vec![0x000A]);
        } else {
            panic!("Expected Data frame");
        }
    }

    #[test]
    fn v3_data() {
        let data = hex::decode(
            "aa030034304758303047503167b2c71d000000000000000000000190012c23e10000000000000000000023e100000000000ae884",
        )
        .unwrap();
        let frame = parse(&data, 0, 0, 11, 1).unwrap();
        if let Frame::Data(df) = frame {
            assert_eq!(df.version, ProtocolVersion::V3);
            assert_eq!(df.idcode, "0GX00GP1");
            assert_eq!(df.analog[0], 0x0190 as f64);
        } else {
            panic!("Expected Data frame");
        }
    }

    #[test]
    fn invalid_sync() {
        let data = hex::decode("bb4200146757dd1d30475830304750310004a5cb").unwrap();
        assert!(parse(&data, 0, 0, 0, 0).is_err());
    }

    #[test]
    fn frame_too_short() {
        let data = hex::decode("aa42").unwrap();
        assert!(parse(&data, 0, 0, 0, 0).is_err());
    }

    /// IDCODE byte preservation: a substation that emits non-ASCII bytes
    /// in the IDCODE slot (e.g. GBK / latin-1 leakage) used to roundtrip
    /// through from_utf8_lossy + as_bytes() to a completely different
    /// 8-byte slot, and the substation would reject every subsequent
    /// command. With latin-1 decode each byte 0xN maps to U+00N and
    /// re-encodes back to byte 0xN.
    #[test]
    fn idcode_byte_roundtrip_non_ascii() {
        use crate::protocol::builder::build_command;
        use crate::protocol::frame::CommandFrame;
        // Command frame with non-ASCII bytes in the IDCODE field.
        let raw_idcode_bytes = [0xC4u8, 0xE3, 0xC1, 0xB9, 0x30, 0x30, 0x30, 0x30];
        let frame = CommandFrame {
            version: ProtocolVersion::V3,
            idcode: raw_idcode_bytes.iter().map(|&b| b as char).collect(),
            soc: 0x67B2C719,
            fracsec: 0,
            cmd: 0x0004,
        };
        let bytes = build_command(&frame).unwrap();
        // IDCODE lives at offset 4..12 in V3 cmd frames.
        assert_eq!(&bytes[4..12], &raw_idcode_bytes, "IDCODE bytes must round-trip");
        // And parse round-trip should preserve the string back.
        let parsed = parse(&bytes, 0, 0, 0, 0).unwrap();
        if let Frame::Command(cmd) = parsed {
            // String::len() counts UTF-8 bytes (latin-1 high-byte chars
            // encode to 2 UTF-8 bytes each), but the logical idcode has
            // 8 chars — one per original byte.
            assert_eq!(cmd.idcode.chars().count(), 8);
            let reencoded: Vec<u8> = cmd.idcode.chars()
                .map(|c| c as u8)
                .collect();
            assert_eq!(reencoded, raw_idcode_bytes);
        } else {
            panic!("expected Command frame");
        }
    }

    /// Frame SIZE < min must be rejected cleanly instead of underflowing
    /// the subsequent `size - 2` index used for CRC.
    #[test]
    fn frame_too_small_size_rejected() {
        // SIZE=0 in a V3 command frame.
        let mut data = hex::decode("aa430018304758303047503167b2c719000000000004ac08").unwrap();
        data[2] = 0; data[3] = 0;
        let err = parse(&data, 0, 0, 0, 0);
        assert!(err.is_err(), "size=0 must error, not panic");
    }

    /// Frame longer than declared SIZE must also be rejected (spec §8:
    /// every frame is exactly SIZE bytes).
    #[test]
    fn frame_extra_trailing_bytes_rejected() {
        let mut data = hex::decode("aa430018304758303047503167b2c719000000000004ac08").unwrap();
        data.push(0xFF);
        assert!(parse(&data, 0, 0, 0, 0).is_err(), "trailing bytes must error");
    }

    #[test]
    fn crc_mismatch() {
        let mut data = hex::decode("aa4200146757dd1d30475830304750310004a5cb").unwrap();
        // Corrupt last 2 bytes
        let len = data.len();
        data[len - 1] = 0xFF;
        data[len - 2] = 0xFF;
        assert!(parse(&data, 0, 0, 0, 0).is_err());
    }

    /// FORMAT 0x000E = analog/phasors/freq all float. Build with the new
    /// format flags, parse with the matching flags, and verify the
    /// engineering values round-trip exactly (within float precision).
    #[test]
    fn v3_data_float_roundtrip() {
        use crate::protocol::builder::build_data;
        use crate::protocol::frame::DataFrame;
        let frame = DataFrame {
            version: ProtocolVersion::V3,
            idcode: "FLOATPMU".into(),
            soc: 0x67B2C719,
            fracsec: 0,
            stat: 0,
            format_flags: 0b1110, // bit1+2+3 = phasor/analog/freq float
            phasors: vec![(123.5, -45.25)],
            freq: 50.123,
            dfreq: -0.05,
            analog: vec![3.14159, -2.71828],
            digital: vec![0xBEEF],
        };
        let bytes = build_data(&frame, 0, 0, 0).unwrap();
        // V3 data header is 20 bytes (sync+size+idcode+soc+fracsec).
        // Float mode: 20 + 2 STAT + 1*8 phasor + 8 (f+df) + 2*4 analog
        //           + 2 digital + 2 CRC = 50
        assert_eq!(bytes.len(), 50);

        let parsed = parse(&bytes, 0b1110, 1, 2, 1).unwrap();
        if let Frame::Data(df) = parsed {
            assert_eq!(df.idcode, "FLOATPMU");
            assert!((df.phasors[0].0 - 123.5).abs() < 1e-3);
            assert!((df.phasors[0].1 - -45.25).abs() < 1e-3);
            assert!((df.freq - 50.123).abs() < 1e-3);
            assert!((df.dfreq - -0.05).abs() < 1e-3);
            assert!((df.analog[0] - 3.14159).abs() < 1e-4);
            assert!((df.analog[1] - -2.71828).abs() < 1e-4);
            assert_eq!(df.digital[0], 0xBEEF);
            assert_eq!(df.format_flags, 0b1110);
        } else {
            panic!("expected Data frame");
        }
    }

    /// NUM_PMU > 1 used to only parse the first block and lose channels
    /// 2..n. Verify roundtrip preserves every block exactly.
    #[test]
    fn v3_config_multi_pmu_roundtrip() {
        use crate::protocol::builder::build_config;
        use crate::protocol::frame::{ConfigFrame, PmuBlock};
        fn mk_block(idx: u16) -> PmuBlock {
            let annmr = 2u16;
            let dgnmr = 1u16;
            let mut channel_names: Vec<String> = (0..annmr).map(|i| format!("PMU{idx}_AN{i}")).collect();
            for j in 0..(dgnmr * 16) { channel_names.push(format!("PMU{idx}_DG{j:02}")); }
            PmuBlock {
                stn: format!("Station{idx}"),
                pmu_idcode: format!("PMU{idx:05}"),
                format_flags: 0,
                phnmr: 0, annmr, dgnmr,
                channel_names,
                phunit: vec![],
                anunit: vec![1000 + idx as u32, 2000 + idx as u32],
                digunit: vec![(0x000F, 0x0000)],
                fnom: 0x0001,
                period: 50 + idx,
            }
        }
        let frame = ConfigFrame {
            version: ProtocolVersion::V3,
            cfg_type: 2,
            idcode: "DC_PMU00".into(),
            soc: 0x67B2C719,
            fracsec: 0,
            d_frame: 0,
            meas_rate: 1_000_000,
            num_pmu: 3,
            // top-level convenience fields will be overwritten by parser
            stn: String::new(), pmu_idcode: String::new(),
            format_flags: 0, phnmr: 0, annmr: 0, dgnmr: 0,
            channel_names: vec![], phunit: vec![], anunit: vec![], digunit: vec![],
            fnom: 0, period: 0,
            pmu_blocks: vec![mk_block(0), mk_block(1), mk_block(2)],
        };
        let bytes = build_config(&frame).unwrap();
        let parsed = parse(&bytes, 0, 0, 0, 0).unwrap();
        if let Frame::Config(cfg) = parsed {
            assert_eq!(cfg.num_pmu, 3);
            assert_eq!(cfg.pmu_blocks.len(), 3);
            for (i, b) in cfg.pmu_blocks.iter().enumerate() {
                let expected = mk_block(i as u16);
                assert_eq!(b.stn, expected.stn, "STN @{i}");
                assert_eq!(b.pmu_idcode, expected.pmu_idcode, "IDCODE @{i}");
                assert_eq!(b.annmr, expected.annmr, "ANNMR @{i}");
                assert_eq!(b.channel_names, expected.channel_names, "CHNAM @{i}");
                assert_eq!(b.anunit, expected.anunit, "ANUNIT @{i}");
                assert_eq!(b.period, expected.period, "PERIOD @{i}");
            }
            // Convenience fields mirror block 0
            assert_eq!(cfg.stn, "Station0");
            assert_eq!(cfg.period, 50);
        } else {
            panic!("expected Config frame");
        }
    }

    /// Parsing a float-mode frame with int16 dims (format_flags=0) would
    /// previously eat the wrong number of bytes — verify the new dispatch
    /// keeps int16 path byte-accurate.
    #[test]
    fn v3_data_int_byte_widths() {
        use crate::protocol::builder::build_data;
        use crate::protocol::frame::DataFrame;
        let frame = DataFrame {
            version: ProtocolVersion::V3,
            idcode: "INTPMU01".into(),
            soc: 0,
            fracsec: 0,
            stat: 0,
            format_flags: 0, // all int16
            phasors: vec![(100.0, 200.0)],
            freq: 10.0,
            dfreq: -5.0,
            analog: vec![1.0, 2.0],
            digital: vec![0xABCD],
        };
        let bytes = build_data(&frame, 0, 0, 0).unwrap();
        // V3 header 20 + STAT 2 + phasor 4 + freq+dfreq 4 + analog 4 +
        // digital 2 + CRC 2 = 38
        assert_eq!(bytes.len(), 38);
    }
}
