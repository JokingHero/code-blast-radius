use blast_radius_engine::analysis::language::{get_language, SupportedLanguage};
use tree_sitter::Parser;

#[test]
fn inspect_julia_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Julia);
    parser
        .set_language(&language)
        .expect("Error loading Julia grammar");

    let code = r#"
        module Physics
            
            struct Particle{T <: Real}
                x::T
            end

            # Short form
            energy(p::Particle) = p.x * 2

            # Where clause + Typed args
            function move!(p::Particle{T}, dx::T) where T
                p.x += dx
            end

            # Defining function on submodule
            function Base.show(io::IO, p::Particle)
                print(io, "P")
            end
        end
    "#;

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    println!("\n--- JULIA S-EXPRESSION ---");
    println!("{}", root.to_sexp());
    println!("--------------------------\n");
}