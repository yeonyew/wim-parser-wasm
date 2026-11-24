use wim_info_wasm::{parse_header, parse_xml, HEADER_SIZE, MAX_XML_BYTES};

fn fixture(xml: &str) -> (Vec<u8>, Vec<u8>) {
    let xml: Vec<u8> = xml.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut header = vec![0u8; HEADER_SIZE];
    header[..8].copy_from_slice(b"MSWIM\0\0\0");
    header[8..12].copy_from_slice(&208u32.to_le_bytes());
    header[12..16].copy_from_slice(&0x10d00u32.to_le_bytes());
    header[40..42].copy_from_slice(&1u16.to_le_bytes());
    header[42..44].copy_from_slice(&1u16.to_le_bytes());
    header[44..48].copy_from_slice(&1u32.to_le_bytes());
    header[72..80].copy_from_slice(&(xml.len() as u64).to_le_bytes());
    header[80..88].copy_from_slice(&208u64.to_le_bytes());
    header[88..96].copy_from_slice(&(xml.len() as u64).to_le_bytes());
    (header, xml)
}

fn parse(xml: &str) -> wim_info_wasm::WimInfo {
    let (h, x) = fixture(xml);
    parse_xml(&parse_header(&h, (h.len() + x.len()) as u64).unwrap(), &x).unwrap()
}

#[test]
fn preserves_decoded_text_and_build_zeroes() {
    let info = parse("\u{feff}<WIM><IMAGE INDEX=\"1\"><NAME> Windows &amp; 테스트 </NAME><DISPLAYNAME><![CDATA[My Windows]]></DISPLAYNAME><WINDOWS><ARCH>009</ARCH><EDITIONID>Professional</EDITIONID><VERSION><BUILD>26100</BUILD><SPBUILD>0011</SPBUILD></VERSION></WINDOWS></IMAGE></WIM>");
    let image = &info.images[0];
    assert_eq!(info.image_count, 1);
    assert_eq!(image.os_name.as_deref(), Some(" Windows & 테스트 "));
    assert_eq!(image.display_name.as_deref(), Some("My Windows"));
    assert_eq!(image.edition.as_deref(), Some("Professional"));
    assert_eq!(image.architecture.as_deref(), Some("009"));
    assert_eq!(image.build_version.as_deref(), Some("26100.0011"));
}

#[test]
fn missing_and_empty_are_different() {
    let info = parse("<WIM><IMAGE INDEX=\"1\"><NAME/></IMAGE></WIM>");
    assert_eq!(info.images[0].os_name.as_deref(), Some(""));
    assert_eq!(info.images[0].display_name, None);
    assert_eq!(info.images[0].architecture, None);
    assert_eq!(info.images[0].build_version, None);
}

#[test]
fn does_not_invent_missing_or_invalid_revision() {
    for version in [
        "<BUILD>26100</BUILD>",
        "<BUILD>26100</BUILD><SPBUILD>bad</SPBUILD>",
        "<BUILD> 26100 </BUILD><SPBUILD>1</SPBUILD>",
    ] {
        let info = parse(&format!(
            "<WIM><IMAGE INDEX=\"1\"><WINDOWS><VERSION>{version}</VERSION></WINDOWS></IMAGE></WIM>"
        ));
        assert!(info.images[0].build_version.is_none());
        assert!(info.images[0].build.is_some());
    }
}

#[test]
fn supports_multiple_images_in_document_order() {
    let (mut h, x) = fixture("<WIM><IMAGE INDEX=\"2\"><NAME>Pro</NAME></IMAGE><IMAGE INDEX=\"1\"><NAME>Home</NAME></IMAGE></WIM>");
    h[44..48].copy_from_slice(&2u32.to_le_bytes());
    let info = parse_xml(&parse_header(&h, (208 + x.len()) as u64).unwrap(), &x).unwrap();
    assert_eq!(
        info.images.iter().map(|i| i.index).collect::<Vec<_>>(),
        vec![2, 1]
    );
}

#[test]
fn rejects_bad_xml_and_inconsistent_indexes() {
    for xml in [
        "<OTHER/>",
        "<WIM>",
        "<WIM/>",
        "<WIM><IMAGE INDEX=\"0\"/></WIM>",
        "<WIM><IMAGE INDEX=\"2\"/></WIM>",
        "<WIM><IMAGE/></WIM>",
        "<WIM><IMAGE INDEX=\"1\"/><IMAGE INDEX=\"1\"/></WIM>",
        "<WIM><IMAGE INDEX=\"1\"><NAME>a</NAME><NAME>b</NAME></IMAGE></WIM>",
        "<WIM><IMAGE INDEX=\"1\"><NAME><B>bad</B></NAME></IMAGE></WIM>",
        "<!DOCTYPE WIM [<!ENTITY x 'a'>]><WIM><IMAGE INDEX=\"1\"><NAME>&x;</NAME></IMAGE></WIM>",
    ] {
        let (h, x) = fixture(xml);
        let header = parse_header(&h, (208 + x.len()) as u64).unwrap();
        assert!(parse_xml(&header, &x).is_err(), "accepted {xml}");
    }
}

#[test]
fn rejects_truncation_bad_utf16_and_wrong_lengths() {
    let (h, mut x) = fixture("<WIM><IMAGE INDEX=\"1\"/></WIM>");
    let header = parse_header(&h, (208 + x.len()) as u64).unwrap();
    assert!(parse_header(&h[..100], 100).is_err());
    assert!(parse_xml(&header, &x[..x.len() - 1]).is_err());
    x[..2].copy_from_slice(&0xd800u16.to_le_bytes());
    assert!(parse_xml(&header, &x).is_err());
}

#[test]
fn rejects_invalid_header_fields() {
    let (h, x) = fixture("<WIM><IMAGE INDEX=\"1\"/></WIM>");
    let size = (208 + x.len()) as u64;
    for (offset, replacement) in [
        (0, vec![0]),
        (8, 100u32.to_le_bytes().to_vec()),
        (12, 0u32.to_le_bytes().to_vec()),
        (42, 2u16.to_le_bytes().to_vec()),
        (44, 0u32.to_le_bytes().to_vec()),
        (79, vec![4]), // compressed XML
        (72, (MAX_XML_BYTES + 2).to_le_bytes().to_vec()),
        (80, u64::MAX.to_le_bytes().to_vec()),
        (88, 0u64.to_le_bytes().to_vec()),
    ] {
        let mut bad = h.clone();
        bad[offset..offset + replacement.len()].copy_from_slice(&replacement);
        assert!(
            parse_header(&bad, size).is_err(),
            "accepted bad offset {offset}"
        );
    }
    assert!(parse_header(&h, size - 1).is_err());
}

#[test]
fn range_can_start_above_four_gibibytes() {
    let (mut h, x) = fixture("<WIM><IMAGE INDEX=\"1\"/></WIM>");
    let offset = (1u64 << 32) + 4096;
    h[80..88].copy_from_slice(&offset.to_le_bytes());
    assert_eq!(
        parse_header(&h, offset + x.len() as u64)
            .unwrap()
            .xml_offset,
        offset
    );
    h[12..16].copy_from_slice(&0xe00u32.to_le_bytes());
    assert!(parse_header(&h, offset + x.len() as u64).is_ok());
}

#[test]
fn reads_all_dism_image_fields_and_preserves_large_numbers() {
    let xml = r#"<WIM><TOTALBYTES>9007199254740993</TOTALBYTES><IMAGE INDEX="1">
<NAME>Windows 11 Pro</NAME><DESCRIPTION>설명</DESCRIPTION><DISPLAYNAME>표시 이름</DISPLAYNAME><DISPLAYDESCRIPTION>표시 설명</DISPLAYDESCRIPTION>
<TOTALBYTES>9007199254740995</TOTALBYTES><HARDLINKBYTES>00123</HARDLINKBYTES><FILECOUNT>123</FILECOUNT><DIRCOUNT>45</DIRCOUNT>
<CREATIONTIME><HIGHPART>0x019DB1DE</HIGHPART><LOWPART>0xD53E8000</LOWPART></CREATIONTIME>
<LASTMODIFICATIONTIME><HIGHPART>0x019DB1DE</HIGHPART><LOWPART>0xD53E8001</LOWPART></LASTMODIFICATIONTIME>
<WIMBOOT>0</WIMBOOT><FLAGS>Professional</FLAGS><WINDOWS>
<ARCH>9</ARCH><PRODUCTNAME>Microsoft Windows Operating System</PRODUCTNAME><EDITIONID>Professional</EDITIONID>
<INSTALLATIONTYPE>Client</INSTALLATIONTYPE><HAL>acpiapic</HAL><PRODUCTTYPE>WinNT</PRODUCTTYPE><PRODUCTSUITE>Terminal Server</PRODUCTSUITE>
<VERSION><MAJOR>10</MAJOR><MINOR>0</MINOR><BUILD>26100</BUILD><SPBUILD>1111</SPBUILD><SPLEVEL>0</SPLEVEL></VERSION>
<SYSTEMROOT>WINDOWS</SYSTEMROOT><LANGUAGES><LANGUAGE>ko-KR</LANGUAGE><LANGUAGE>en-US</LANGUAGE><DEFAULT>ko-KR</DEFAULT><FALLBACK LANGUAGE="ko-KR">en-US</FALLBACK></LANGUAGES>
</WINDOWS></IMAGE></WIM>"#;
    let (mut h, x) = fixture(xml);
    h[120..124].copy_from_slice(&1u32.to_le_bytes());
    let result = parse_xml(&parse_header(&h, (208 + x.len()) as u64).unwrap(), &x).unwrap();
    let image = &result.images[0];
    assert_eq!(result.total_bytes.as_deref(), Some("9007199254740993"));
    assert_eq!(image.image_size.as_deref(), Some("9007199254740995"));
    assert_eq!(image.hard_link_bytes.as_deref(), Some("00123"));
    assert_eq!(image.description.as_deref(), Some("설명"));
    assert_eq!(image.display_description.as_deref(), Some("표시 설명"));
    assert_eq!(
        image.product_name.as_deref(),
        Some("Microsoft Windows Operating System")
    );
    assert_eq!(image.installation_type.as_deref(), Some("Client"));
    assert_eq!(image.hal.as_deref(), Some("acpiapic"));
    assert_eq!(image.product_type.as_deref(), Some("WinNT"));
    assert_eq!(image.product_suite.as_deref(), Some("Terminal Server"));
    assert_eq!(image.version.as_deref(), Some("10.0.26100"));
    assert_eq!(image.sp_level.as_deref(), Some("0"));
    assert_eq!(image.system_root.as_deref(), Some("WINDOWS"));
    assert_eq!(image.file_count.as_deref(), Some("123"));
    assert_eq!(image.directory_count.as_deref(), Some("45"));
    assert_eq!(image.wim_boot.as_deref(), Some("0"));
    assert_eq!(image.flags.as_deref(), Some("Professional"));
    assert!(image.bootable);
    let languages = image.languages.as_ref().unwrap();
    assert_eq!(languages.values, vec!["ko-KR", "en-US"]);
    assert_eq!(languages.count, 2);
    assert_eq!(languages.default.as_deref(), Some("ko-KR"));
    assert_eq!(languages.default_index, Some(0));
    assert_eq!(languages.fallbacks[0].language.as_deref(), Some("ko-KR"));
    assert_eq!(languages.fallbacks[0].value, "en-US");
    let created = image.created_time.as_ref().unwrap();
    assert_eq!(created.high_part.as_deref(), Some("0x019DB1DE"));
    assert_eq!(created.utc.as_deref(), Some("1970-01-01T00:00:00Z"));
    assert_eq!(
        image.modified_time.as_ref().unwrap().utc.as_deref(),
        Some("1970-01-01T00:00:00.0000001Z")
    );
}

#[test]
fn reads_container_resources_without_truncating_u64() {
    let (mut h, x) = fixture("<WIM><IMAGE INDEX=\"1\"/></WIM>");
    h[16..20].copy_from_slice(&0x40002u32.to_le_bytes());
    h[20..24].copy_from_slice(&32768u32.to_le_bytes());
    h[24..40].copy_from_slice(&[
        0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ]);
    h[48..56].copy_from_slice(&((4u64 << 56) | 123).to_le_bytes());
    h[56..64].copy_from_slice(&9_007_199_254_740_993u64.to_le_bytes());
    h[64..72].copy_from_slice(&456u64.to_le_bytes());
    h[124..132].copy_from_slice(&12u64.to_le_bytes());
    let c = parse_header(&h, (208 + x.len()) as u64).unwrap().container;
    assert_eq!(c.flags, 0x40002);
    assert_eq!(c.compression_chunk_size, 32768);
    assert_eq!(c.guid, "00112233-4455-6677-8899-aabbccddeeff");
    assert_eq!(c.lookup_table.offset, "9007199254740993");
    assert_eq!(c.lookup_table.size, "123");
    assert_eq!(c.lookup_table.original_size, "456");
    assert_eq!(c.lookup_table.flags, 4);
    assert!(c.has_integrity_table);
}

#[test]
fn optional_details_remain_missing_or_raw_when_unusable() {
    let result = parse(
        r#"<WIM><IMAGE INDEX="1"><CREATIONTIME><HIGHPART>invalid</HIGHPART></CREATIONTIME><WINDOWS><LANGUAGES><DEFAULT>unknown</DEFAULT></LANGUAGES></WINDOWS></IMAGE></WIM>"#,
    );
    let i = &result.images[0];
    assert_eq!(
        i.created_time.as_ref().unwrap().high_part.as_deref(),
        Some("invalid")
    );
    assert_eq!(i.created_time.as_ref().unwrap().utc, None);
    assert_eq!(i.modified_time, None);
    assert_eq!(i.version, None);
    assert!(!i.bootable);
    assert_eq!(i.languages.as_ref().unwrap().default_index, None);
    assert_eq!(i.languages.as_ref().unwrap().count, 0);
}
