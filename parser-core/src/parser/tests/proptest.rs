use proptest::prelude::*;

use crate::{
    domain::{ParserContext, SlashParseResult},
    parser::parse_to_domain,
    serialize::to_json,
    to_plaintext::to_plaintext,
};

proptest! {
    #[test]
    fn serialization_never_panics(original in any::<SlashParseResult>()) {
        // Arbitrary ASTs should always serialize without panicking
        let _json = to_json(&original)
            .expect("Valid domain object should always serialize");
    }

    #[test]
    fn parse_to_plaintext_roundtrip(input in "[\\s\\S]{0,200}") {
        // The real roundtrip property:
        // parse(input) -> to_plaintext -> parse again -> same structure
        let ctx1 = ParserContext::default();
        let ast1 = parse_to_domain(&input, ctx1);

        let plaintext = to_plaintext(&ast1);

        let ctx2 = ParserContext::default();
        let ast2 = parse_to_domain(&plaintext, ctx2);

        prop_assert_eq!(
            ast1.commands.len(),
            ast2.commands.len(),
            "Command count mismatch.\nInput: {:?}\nPlaintext: {:?}",
            input,
            plaintext
        );

        for (a, b) in ast1.commands.iter().zip(ast2.commands.iter()) {
            prop_assert_eq!(&a.name, &b.name, "Command name mismatch");
            prop_assert_eq!(&a.arguments.mode, &b.arguments.mode, "Argument mode mismatch");
            prop_assert_eq!(&a.arguments.payload, &b.arguments.payload, "Payload mismatch");
        }

        prop_assert_eq!(
            ast1.text_blocks.len(),
            ast2.text_blocks.len(),
            "Text block count mismatch.\nInput: {:?}\nPlaintext: {:?}",
            input,
            plaintext
        );
    }
}
