use syn::{Expr, Lit};

pub trait TryToIntLiteral {
    fn try_to_int_literal(&self) -> Option<&str>;
}

impl TryToIntLiteral for Expr {
    fn try_to_int_literal(&self) -> Option<&str> {
        let Expr::Lit(lit) = self else {return None};
        lit.lit.try_to_int_literal()
    }
}

impl TryToIntLiteral for Lit {
    fn try_to_int_literal(&self) -> Option<&str> {
        let Lit::Int(int_lit) = self else {return None};
        Some(int_lit.base10_digits())
    }
}
