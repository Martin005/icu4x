// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::error::*;
use crate::options::*;
use crate::provider::*;
use alloc::string::String;
use formatted_string::*;
use icu_locid::Locale;
use icu_provider::prelude::*;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FieldType {
    Element,
    Literal,
}

pub struct ListFormatter {
    data: DataPayload<ListFormatterPatternsV1Marker>,
    width: Width,
}

impl ListFormatter {
    pub fn try_new<T: Into<Locale>, D: DataProvider<ListFormatterPatternsV1Marker> + ?Sized>(
        locale: T,
        data_provider: &D,
        type_: Type,
        width: Width,
    ) -> Result<Self, Error> {
        let data = data_provider
            .load_payload(&DataRequest {
                resource_path: ResourcePath {
                    key: match type_ {
                        Type::And => key::LIST_FORMAT_AND_V1,
                        Type::Or => key::LIST_FORMAT_OR_V1,
                        Type::Unit => key::LIST_FORMAT_UNIT_V1,
                    },
                    options: ResourceOptions {
                        variant: None,
                        langid: Some(locale.into().into()),
                    },
                },
            })?
            .take_payload()?;
        Ok(Self { data, width })
    }

    fn format_internal<'c, B>(
        &'c self,
        values: &[&str],
        // Empty builder
        mut builder: B,
        // Produces value + builder
        apply_value: fn(&str, B) -> B,
        // Produces value + parts.between + builder + parts.after
        apply_pattern: fn(&str, PatternParts<'c>, B) -> B,
    ) -> B {
        match values.len() {
            0 => builder,
            1 => apply_value(values[0], builder),
            2 => apply_pattern(
                values[0],
                self.data.get().pair(self.width).parts(values[1]),
                apply_value(values[1], builder),
            ),
            n => {
                builder = apply_pattern(
                    values[n - 2],
                    self.data.get().end(self.width).parts(values[n - 1]),
                    apply_value(values[n - 1], builder),
                );
                let middle = self.data.get().middle(self.width);
                for i in (1..n - 2).rev() {
                    builder = apply_pattern(values[i], middle.parts(values[i + 1]), builder);
                }
                apply_pattern(
                    values[0],
                    self.data.get().start(self.width).parts(values[1]),
                    builder,
                )
            }
        }
    }

    fn size_hint(&self, values: &[&str]) -> usize {
        values.iter().map(|s| s.len()).sum::<usize>()
            + self.data.get().size_hint(self.width, values.len())
    }

    pub fn format(&self, values: &[&str]) -> String {
        self.format_internal(
            values,
            String::with_capacity(self.size_hint(values)),
            |value, mut builder| {
                builder.push_str(value);
                builder
            },
            |value, (between, after), mut builder| {
                // TODO(robertbastian): String doesn't have O(1) prepend
                builder.insert_str(0, between);
                builder.insert_str(0, value);
                builder + after
            },
        )
    }

    pub fn format_to_parts(&self, values: &[&str]) -> FormattedString<FieldType> {
        self.format_internal(
            values,
            FormattedString::with_capacity(self.size_hint(values)),
            |value, mut builder| {
                builder.append(&value, FieldType::Element);
                builder
            },
            |value, (between, after), mut builder| {
                // TODO(robertbastian): FormattedString doesn't have O(1) prepend
                builder.prepend(&between, FieldType::Literal);
                builder.prepend(&value, FieldType::Element);
                builder.append(&after, FieldType::Literal);
                builder
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALUES: &[&str] = &["one", "two", "three", "four", "five"];

    fn formatter(width: Width) -> ListFormatter {
        ListFormatter {
            data: DataPayload::<ListFormatterPatternsV1Marker>::from_owned(
                crate::provider::test::test_patterns(),
            ),
            width,
        }
    }

    #[test]
    fn test_format() {
        let formatter = formatter(Width::Wide);
        assert_eq!(formatter.format(&VALUES[0..0]), "");
        assert_eq!(formatter.format(&VALUES[0..1]), "one");
        assert_eq!(formatter.format(&VALUES[0..2]), "one; two");
        assert_eq!(formatter.format(&VALUES[0..3]), "one: two. three!");
        assert_eq!(formatter.format(&VALUES[0..4]), "one: two, three. four!");
        assert_eq!(formatter.format(VALUES), "one: two, three, four. five!");
    }

    #[test]
    fn test_format_to_parts() {
        let formatter = formatter(Width::Wide);

        assert_eq!(formatter.format_to_parts(&VALUES[0..0]).as_ref(), "");
        assert_eq!(formatter.format_to_parts(&VALUES[0..1]).as_ref(), "one");
        assert_eq!(
            formatter.format_to_parts(&VALUES[0..2]).as_ref(),
            "one; two"
        );
        assert_eq!(
            formatter.format_to_parts(&VALUES[0..3]).as_ref(),
            "one: two. three!"
        );
        assert_eq!(
            formatter.format_to_parts(&VALUES[0..4]).as_ref(),
            "one: two, three. four!"
        );
        let parts = formatter.format_to_parts(VALUES);
        assert_eq!(parts.as_ref(), "one: two, three, four. five!");

        assert_eq!(parts.fields_at(0), [FieldType::Element]);
        assert!(parts.is_field_start(0, 0));
        assert_eq!(parts.fields_at(2), [FieldType::Element]);
        assert!(!parts.is_field_start(2, 0));
        assert_eq!(parts.fields_at(3), [FieldType::Literal]);
        assert!(parts.is_field_start(3, 0));
        assert_eq!(parts.fields_at(4), [FieldType::Literal]);
        assert!(!parts.is_field_start(4, 0));
        assert_eq!(parts.fields_at(5), [FieldType::Element]);
        assert!(parts.is_field_start(5, 0));
    }

    #[test]
    fn test_conditional() {
        let formatter = formatter(Width::Narrow);

        assert_eq!(formatter.format(&["Beta", "Alpha"]), "Beta :o Alpha");
    }

    #[test]
    fn strings_dont_have_spare_capacity() {
        let string = formatter(Width::Short).format(VALUES);
        assert_eq!(string.capacity(), string.len());

        let labelled_string = formatter(Width::Short).format(VALUES);
        assert_eq!(labelled_string.capacity(), labelled_string.len());
    }
}
