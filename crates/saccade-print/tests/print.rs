//! Generated raster acceptance and negative input contracts.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;
use saccade_print::{Options, compare};
#[test]
fn cross_domain_identity_shift_tac_and_marks() {
    let dir = tempfile::tempdir().unwrap();
    let profile = support::icc();
    let target = dir.path().join("synthetic.icc");
    std::fs::write(&target, &profile).unwrap();
    for domain in ["packaging-label", "magazine-page"] {
        let (a, b) = support::fixture(dir.path(), domain);
        let out = dir.path().join(format!("{domain}-identity"));
        std::fs::create_dir(&out).unwrap();
        let options = Options {
            output_profile: Some(target.clone()),
            ..Options::default()
        };
        let identity = compare(&a, &a, &out, &options).unwrap();
        assert_eq!(identity["delta_e2000"]["max"], 0.);
        assert!(
            identity["report_id"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        let out = dir.path().join(format!("{domain}-shift"));
        std::fs::create_dir(&out).unwrap();
        let changed = compare(&a, &b, &out, &options).unwrap();
        let schema: serde_json::Value = serde_json::from_str(saccade_print::JSON_SCHEMA).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let errors: Vec<_> = validator
            .iter_errors(&changed)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
        let mut invalid = changed.clone();
        invalid["delta_e2000"]["max"] = (-1).into();
        assert!(!validator.is_valid(&invalid));
        let max = changed["delta_e2000"]["max"].as_f64().unwrap();
        assert!((2.4..2.5).contains(&max), "{domain}: {max}");
        assert_eq!(changed["sides"]["candidate"]["over_limit_pixels"], 50);
        assert!(
            !changed["sides"]["candidate"]["registration_sensitive_marks"]["candidates"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_ne!(identity["report_id"], changed["report_id"]);
        let mask = image::open(out.join("candidate-tac-over.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(mask.get_pixel(8, 35)[0], 255);
        assert_eq!(mask.get_pixel(0, 0)[0], 0);
        assert_eq!(changed["separations_percent"]["k"]["max"], 0.);
        assert!(changed["separations_percent"]["c"]["max"].as_f64().unwrap() > 7.);
        println!("{domain}: delta max={max}, TAC=50, identity=0");
    }
}
#[test]
fn rgb_missing_and_invalid_profiles_are_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let rgb = dir.path().join("rgb.png");
    image::RgbImage::new(2, 2).save(&rgb).unwrap();
    assert_eq!(
        compare(&rgb, &rgb, dir.path(), &Options::default())
            .unwrap_err()
            .code(),
        "not_print_input"
    );
    let cmyk = dir.path().join("cmyk.tif");
    support::tiff(&cmyk, 2, 2, &[0; 16], None);
    assert_eq!(
        compare(&cmyk, &cmyk, dir.path(), &Options::default())
            .unwrap_err()
            .code(),
        "print_profile"
    );
    let icc = dir.path().join("input.icc");
    std::fs::write(&icc, support::icc()).unwrap();
    let options = Options {
        input_profile: Some(icc.clone()),
        ..Options::default()
    };
    assert_eq!(
        compare(&cmyk, &cmyk, dir.path(), &options).unwrap()["profiles"]["reference"]["source"],
        "explicit"
    );
    std::fs::write(&icc, b"invalid").unwrap();
    assert_eq!(
        compare(&cmyk, &cmyk, dir.path(), &options)
            .unwrap_err()
            .code(),
        "print_profile"
    );
}
#[test]
fn delta_e_reference_pair() {
    use lcms2::CIELabExt;
    let a = lcms2::CIELab {
        L: 50.,
        a: 2.6772,
        b: -79.7751,
    };
    let b = lcms2::CIELab {
        L: 50.,
        a: 0.,
        b: -82.7485,
    };
    assert!((a.cie2000_delta_e(&b, 1., 1., 1.) - 2.0425).abs() < 0.0001);
}
#[test]
fn alpha_coverage_and_16_bit_tiff() {
    use tiff::{
        encoder::{TiffEncoder, colortype},
        tags::Tag,
    };
    let dir = tempfile::tempdir().unwrap();
    let profile = support::icc();
    let alpha = dir.path().join("alpha.tif");
    let mut enc = TiffEncoder::new(std::fs::File::create(&alpha).unwrap()).unwrap();
    let mut img = enc.new_image::<colortype::CMYKA8>(2, 1).unwrap();
    img.encoder()
        .write_tag(Tag::IccProfile, profile.as_slice())
        .unwrap();
    img.encoder()
        .write_tag(Tag::ExtraSamples, &[2_u16][..])
        .unwrap();
    img.write_data(&[255, 255, 255, 255, 0, 0, 0, 0, 255, 255])
        .unwrap();
    drop(enc);
    let opaque = dir.path().join("opaque.tif");
    support::tiff(&opaque, 2, 1, &[0, 0, 0, 0, 0, 0, 0, 255], Some(&profile));
    let report = compare(&alpha, &opaque, dir.path(), &Options::default()).unwrap();
    assert_eq!(report["delta_e2000"]["max"], 0.);
    let sixteen = dir.path().join("sixteen.tif");
    let mut enc = TiffEncoder::new(std::fs::File::create(&sixteen).unwrap()).unwrap();
    let mut img = enc.new_image::<colortype::CMYK16>(2, 1).unwrap();
    img.encoder()
        .write_tag(Tag::IccProfile, profile.as_slice())
        .unwrap();
    img.write_data(&[0, 0, 0, 0, 0, 0, 0, 65535]).unwrap();
    drop(enc);
    assert_eq!(
        compare(&sixteen, &opaque, dir.path(), &Options::default()).unwrap()["delta_e2000"]["max"],
        0.
    );
}
#[test]
fn raster_pdf_preserves_profile_and_refuses_vector_content() {
    use lopdf::{Document, Object, Stream, dictionary};
    let dir = tempfile::tempdir().unwrap();
    let profile = support::icc();
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let icc = doc.add_object(Stream::new(dictionary! {"N"=>4}, profile.clone()));
    let image=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>2,"Height"=>1,"BitsPerComponent"=>8,"ColorSpace"=>vec![Object::Name(b"ICCBased".to_vec()),icc.into()]},vec![0,0,0,0,0,0,0,255]));
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"q 2 0 0 1 0 0 cm /Im0 Do Q".to_vec(),
    ));
    let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),2.into(),1.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Im0"=>image}},"Contents"=>content});
    doc.objects.insert(
        pages,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", root);
    let pdf = dir.path().join("raster.pdf");
    doc.save(&pdf).unwrap();
    let tiff = dir.path().join("raster.tif");
    support::tiff(&tiff, 2, 1, &[0, 0, 0, 0, 0, 0, 0, 255], Some(&profile));
    assert_eq!(
        compare(&pdf, &tiff, dir.path(), &Options::default()).unwrap()["delta_e2000"]["max"],
        0.
    );
    doc.compress();
    doc.save(&pdf).unwrap();
    assert_eq!(
        compare(&pdf, &tiff, dir.path(), &Options::default()).unwrap()["delta_e2000"]["max"],
        0.
    );
    let stream = doc
        .get_object_mut(content)
        .unwrap()
        .as_stream_mut()
        .unwrap();
    stream.dict.remove(b"Filter");
    stream.set_content(b"q 2 0 0 1 0 0 cm /Im0 Do Q 0 0 1 1 re f".to_vec());
    doc.save(&pdf).unwrap();
    assert_eq!(
        compare(&pdf, &tiff, dir.path(), &Options::default())
            .unwrap_err()
            .code(),
        "print_unsupported"
    );
}
#[test]
fn adobe_cmyk_jpeg_embedded_profile_and_ink_polarity() {
    let dir = tempfile::tempdir().unwrap();
    let icc = support::icc();
    let source = include_bytes!("fixtures/constant-cmyk.jpg");
    let mut jpeg = source[..2].to_vec();
    jpeg.extend([0xff, 0xe2]);
    jpeg.extend(((icc.len() + 16) as u16).to_be_bytes());
    jpeg.extend(b"ICC_PROFILE\0\x01\x01");
    jpeg.extend(&icc);
    jpeg.extend(&source[2..]);
    let jpg = dir.path().join("cmyk.jpg");
    std::fs::write(&jpg, jpeg).unwrap();
    let tif = dir.path().join("cmyk.tif");
    support::tiff(&tif, 8, 8, &[64, 32, 16, 128].repeat(64), Some(&icc));
    let report = compare(&jpg, &tif, dir.path(), &Options::default()).unwrap();
    assert!(report["delta_e2000"]["max"].as_f64().unwrap() < 0.01);
    assert!(report["separations_percent"]["k"]["max"].as_f64().unwrap() < 0.01);
}

#[test]
fn target_gamut_changes_report_and_locates_colour_region() {
    let dir = tempfile::tempdir().unwrap();
    let (a, _) = support::fixture(dir.path(), "packaging-label");
    let target = dir.path().join("narrow.icc");
    std::fs::write(&target, support::icc_gamut(8.)).unwrap();
    let options = Options {
        output_profile: Some(target),
        ..Options::default()
    };
    let report = compare(&a, &a, dir.path(), &options).unwrap();
    assert!(
        report["sides"]["reference"]["out_of_gamut"]["share"]
            .as_f64()
            .unwrap()
            > 0.1
    );
    let mask = image::open(dir.path().join("reference-out-of-gamut.png"))
        .unwrap()
        .to_luma8();
    assert_eq!(mask.get_pixel(10, 10)[0], 255);
    assert_eq!(mask.get_pixel(0, 0)[0], 0);
}

#[test]
fn separate_embedded_profiles_are_honoured_and_override_is_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.tif");
    let b = dir.path().join("b.tif");
    support::tiff(&a, 1, 1, &[180, 30, 80, 15], Some(&support::icc()));
    support::tiff(&b, 1, 1, &[180, 30, 80, 15], Some(&support::icc_gamut(40.)));
    let report = compare(&a, &b, dir.path(), &Options::default()).unwrap();
    assert!(report["delta_e2000"]["max"].as_f64().unwrap() > 5.);
    let profile = dir.path().join("override.icc");
    std::fs::write(&profile, support::icc()).unwrap();
    let options = Options {
        input_profile: Some(profile),
        ..Options::default()
    };
    let overridden = compare(&a, &b, dir.path(), &options).unwrap();
    assert_eq!(overridden["delta_e2000"]["max"], 0.);
    assert_ne!(
        overridden["profiles"]["reference"]["embedded_sha256"],
        overridden["profiles"]["candidate"]["embedded_sha256"]
    );
    assert_eq!(
        overridden["profiles"]["reference"]["sha256"],
        overridden["profiles"]["candidate"]["sha256"]
    );
    assert_ne!(report["report_id"], overridden["report_id"]);
}
