// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `FeatureSerializerTest.kt`.

use super::super::svg::FeatureSerializer;
use super::super::*;
use super::utils::*;

use alloc::vec;

#[test]
fn throws_for_empty_parse() {
    let serialized = "";

    assert!(FeatureSerializer::parse(serialized).is_err());
}

#[test]
fn throws_for_blank_parse() {
    let serialized = "                    ";

    assert!(FeatureSerializer::parse(serialized).is_err());
}

#[test]
fn throws_for_only_version_no_tags() {
    let serialized = "V1                    ";

    assert!(FeatureSerializer::parse(serialized).is_err());
}

#[test]
fn throws_for_insufficient_coordinate_count() {
    let serialized0 = "V1c";
    let serialized1 = "V1o1,1,2,2";
    let serialized2 = "V1o1,1,2,2,3,3,4,4,5,5,6,6";
    let serialized3 = "V1o1,1,2,2,3,3,4,4n4,4";

    assert!(FeatureSerializer::parse(serialized0).is_err());
    assert!(FeatureSerializer::parse(serialized1).is_err());
    assert!(FeatureSerializer::parse(serialized2).is_err());
    assert!(FeatureSerializer::parse(serialized3).is_err());
}

#[test]
fn throws_for_wrong_separator() {
    let serialized = "V1o1 1 2 2 3 3 4 4";

    assert!(FeatureSerializer::parse(serialized).is_err());
}

#[test]
fn throws_for_non_numbers() {
    let serialized = "V1o1,1,two,2,3,three,4,4";

    assert!(FeatureSerializer::parse(serialized).is_err());
}

#[test]
fn throws_when_tag_not_first() {
    let serialized = "V11,1,2,2,3,4,4,4o";

    assert!(FeatureSerializer::parse(serialized).is_err());
}

#[test]
fn throws_when_coordinates_and_tag_separated() {
    let serialized = "V1o,1,1,2,2,3,4,4,4";

    assert!(FeatureSerializer::parse(serialized).is_err());
}

#[test]
fn treats_unknown_feature_tag_as_edge() {
    let serialized = "V1 h1,1,2,2,3,3,4,4";
    let expected = Feature::Edge(vec![Cubic::from_floats(1., 1., 2., 2., 3., 3., 4., 4.)]);

    assert_features_equalish(&expected, &FeatureSerializer::parse(serialized).unwrap()[0]);
}

#[test]
fn treats_no_version_as_v1() {
    let serialized_no_version = "n1,1,2,2,3,3,4,4x4,4,5,5,6,6,7,7o7,7,8,8,9,9,10,10";
    let serialized_v1 = "V1 n1,1,2,2,3,3,4,4x4,4,5,5,6,6,7,7o7,7,8,8,9,9,10,10";

    let no_version_parse = FeatureSerializer::parse(serialized_no_version).unwrap();
    let v1_parse = FeatureSerializer::parse(serialized_v1).unwrap();

    assert_eq!(no_version_parse.len(), v1_parse.len());
    for index in 0..v1_parse.len() {
        assert_features_equalish(&v1_parse[index], &no_version_parse[index]);
    }
}

#[test]
fn treats_unknown_versions_as_v1() {
    let serialized_v999 = "V999 n1,1,2,2,3,3,4,4x4,4,5,5,6,6,7,7o7,7,8,8,9,9,10,10";
    let serialized_v1 = "V1 n1,1,2,2,3,3,4,4x4,4,5,5,6,6,7,7o7,7,8,8,9,9,10,10";

    let v999_parse = FeatureSerializer::parse(serialized_v999).unwrap();
    let v1_parse = FeatureSerializer::parse(serialized_v1).unwrap();

    assert_eq!(v999_parse.len(), v1_parse.len());

    for index in 0..v1_parse.len() {
        assert_features_equalish(&v1_parse[index], &v999_parse[index]);
    }
}

#[test]
fn ignores_excess_spaces() {
    let serialized = "V1       n  1 ,  1  ,2  ,  2 , 3  ,  3  , 4 , 4   ";
    let expected = Feature::Edge(vec![Cubic::from_floats(1., 1., 2., 2., 3., 3., 4., 4.)]);

    assert_features_equalish(&expected, &FeatureSerializer::parse(serialized).unwrap()[0]);
}

#[test]
fn parses_edge_with_single_cubic() {
    let serialized = "V1 n1,1,2,2,3,3,4,4";
    let expected = Feature::Edge(vec![Cubic::from_floats(1., 1., 2., 2., 3., 3., 4., 4.)]);

    assert_features_equalish(&expected, &FeatureSerializer::parse(serialized).unwrap()[0]);
}

#[test]
fn parses_edge_with_single_cubic_and_floats() {
    let serialized = "V1n1.1,1.1,2.12,2.12,3.123,3.123,4.1234,4.1234";
    let expected =
        Feature::Edge(vec![Cubic::from_floats(1.1, 1.1, 2.12, 2.12, 3.123, 3.123, 4.1234, 4.1234)]);

    assert_features_equalish(&expected, &FeatureSerializer::parse(serialized).unwrap()[0]);
}

#[test]
fn parses_convex_corner_with_single_cubic() {
    let serialized = "V1x1,1,2,2,3,3,4,4";
    let expected = Feature::Corner {
        cubics: vec![Cubic::from_floats(1., 1., 2., 2., 3., 3., 4., 4.)],
        convex: true,
    };

    assert_features_equalish(&expected, &FeatureSerializer::parse(serialized).unwrap()[0]);
}

#[test]
fn parses_concave_corner_with_single_cubic() {
    let serialized = "V1o1,1,2,2,3,3,4,4";
    let expected = Feature::Corner {
        cubics: vec![Cubic::from_floats(1., 1., 2., 2., 3., 3., 4., 4.)],
        convex: false,
    };

    assert_features_equalish(&expected, &FeatureSerializer::parse(serialized).unwrap()[0]);
}

#[test]
fn parses_edge_with_multiple_cubics() {
    let serialized = "V1n1,1,2,2,3,3,4,4,5,5,6,6,7,7";
    let expected = Feature::Edge(vec![
        Cubic::from_floats(1., 1., 2., 2., 3., 3., 4., 4.),
        Cubic::from_floats(4., 4., 5., 5., 6., 6., 7., 7.),
    ]);

    let actual = FeatureSerializer::parse(serialized).unwrap();

    assert_features_equalish(&expected, &actual[0]);
}

#[test]
fn parses_convex_corner_with_multiple_cubics() {
    let serialized = "V1x1,1,2,2,3,3,4,4,5,5,6,6,7,7";
    let expected = Feature::Corner {
        cubics: vec![
            Cubic::from_floats(1., 1., 2., 2., 3., 3., 4., 4.),
            Cubic::from_floats(4., 4., 5., 5., 6., 6., 7., 7.),
        ],
        convex: true,
    };

    let actual = FeatureSerializer::parse(serialized).unwrap();

    assert_features_equalish(&expected, &actual[0]);
}

#[test]
fn parses_concave_corner_with_multiple_cubics() {
    let serialized = "V1o1,1,2,2,3,3,4,4,5,5,6,6,7,7";
    let expected = Feature::Corner {
        cubics: vec![
            Cubic::from_floats(1., 1., 2., 2., 3., 3., 4., 4.),
            Cubic::from_floats(4., 4., 5., 5., 6., 6., 7., 7.),
        ],
        convex: false,
    };

    let actual = FeatureSerializer::parse(serialized).unwrap();

    assert_features_equalish(&expected, &actual[0]);
}

#[test]
fn parses_convex_corner_with_a_lot_of_cubics() {
    let serialized =
        "V1x1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13,14,14,15,15,16,16";
    let expected = Feature::Corner {
        cubics: vec![
            Cubic::from_floats(1., 1., 2., 2., 3., 3., 4., 4.),
            Cubic::from_floats(4., 4., 5., 5., 6., 6., 7., 7.),
            Cubic::from_floats(7., 7., 8., 8., 9., 9., 10., 10.),
            Cubic::from_floats(10., 10., 11., 11., 12., 12., 13., 13.),
            Cubic::from_floats(13., 13., 14., 14., 15., 15., 16., 16.),
        ],
        convex: true,
    };

    let actual = FeatureSerializer::parse(serialized).unwrap();

    assert_features_equalish(&expected, &actual[0]);
}

#[test]
fn parses_multiple_features_with_multiple_cubics() {
    let serialized = "V1n1,1,2,2,3,3,4,4x4,4,5,5,6,6,7,7o7,7,8,8,9,9,10,10";
    let expected = [
        Feature::Edge(vec![Cubic::from_floats(1., 1., 2., 2., 3., 3., 4., 4.)]),
        Feature::Corner {
            cubics: vec![Cubic::from_floats(4., 4., 5., 5., 6., 6., 7., 7.)],
            convex: true,
        },
        Feature::Corner {
            cubics: vec![Cubic::from_floats(7., 7., 8., 8., 9., 9., 10., 10.)],
            convex: false,
        },
    ];

    let actual = FeatureSerializer::parse(serialized).unwrap();

    assert_eq!(expected.len(), actual.len());
    for index in 0..expected.len() {
        assert_features_equalish(&expected[index], &actual[index]);
    }
}

#[test]
fn serializes_edge_with_single_cubic() {
    let expected = "V1n1,1,2,2,3,3,4,4";
    let feature = FeatureSerializer::parse(expected).unwrap();
    let actual = FeatureSerializer::serialize(&feature);

    assert_eq!(expected, actual.as_str());
}

#[test]
fn serializes_edge_with_single_cubic_and_floats() {
    let expected = "V1n1.1,1.1,2.12,2.12,3.123,3.123,4.1234,4.1234";
    let feature = FeatureSerializer::parse(expected).unwrap();
    let actual = FeatureSerializer::serialize(&feature);

    assert_eq!(expected, actual.as_str());
}

#[test]
fn serializes_convex_corner_with_single_cubic() {
    let expected = "V1x1,1,2,2,3,3,4,4";
    let feature = FeatureSerializer::parse(expected).unwrap();
    let actual = FeatureSerializer::serialize(&feature);

    assert_eq!(expected, actual.as_str());
}

#[test]
fn serializes_concave_corner_with_single_cubic() {
    let expected = "V1o1,1,2,2,3,3,4,4";
    let feature = FeatureSerializer::parse(expected).unwrap();
    let actual = FeatureSerializer::serialize(&feature);

    assert_eq!(expected, actual.as_str());
}

#[test]
fn serializes_edge_with_multiple_cubics() {
    let expected = "V1n1,1,2,2,3,3,4,4,5,5,6,6,7,7";
    let feature = FeatureSerializer::parse(expected).unwrap();
    let actual = FeatureSerializer::serialize(&feature);

    assert_eq!(expected, actual.as_str());
}

#[test]
fn serializes_convex_corner_with_multiple_cubics() {
    let expected = "V1x1,1,2,2,3,3,4,4,5,5,6,6,7,7";
    let feature = FeatureSerializer::parse(expected).unwrap();
    let actual = FeatureSerializer::serialize(&feature);

    assert_eq!(expected, actual.as_str());
}

#[test]
fn serializes_concave_corner_with_multiple_cubics() {
    let expected = "V1o1,1,2,2,3,3,4,4,5,5,6,6,7,7";
    let feature = FeatureSerializer::parse(expected).unwrap();
    let actual = FeatureSerializer::serialize(&feature);

    assert_eq!(expected, actual.as_str());
}

#[test]
fn serializes_convex_corner_with_a_lot_of_cubics() {
    let expected =
        "V1x1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13,14,14,15,15,16,16";
    let feature = FeatureSerializer::parse(expected).unwrap();
    let actual = FeatureSerializer::serialize(&feature);

    assert_eq!(expected, actual.as_str());
}

#[test]
fn serializes_multiple_features_with_multiple_cubics() {
    let expected = "V1n1,1,2,2,3,3,4,4x4,4,5,5,6,6,7,7o7,7,8,8,9,9,10,10";
    let features = FeatureSerializer::parse(expected).unwrap();
    let actual = FeatureSerializer::serialize(&features);

    assert_eq!(expected, actual.as_str());
}
