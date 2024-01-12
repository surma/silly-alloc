use syn::{Expr, ExprType, Lit};

pub trait TryToIntLiteral {
    fn try_to_int_literal(&self) -> Option<&str>;
}

impl TryToIntLiteral for Expr {
    fn try_to_int_literal(&self) -> Option<&str> {
        let Expr::Lit(lit) = self else { return None };
        lit.lit.try_to_int_literal()
    }
}

pub trait TryToTypeExpression {
    fn try_to_type_expression(&self) -> Option<&ExprType>;
}

impl TryToTypeExpression for Expr {
    fn try_to_type_expression(&self) -> Option<&ExprType> {
        let Expr::Type(type_expr) = self else {
            return None;
        };
        Some(type_expr)
    }
}

impl TryToIntLiteral for Lit {
    fn try_to_int_literal(&self) -> Option<&str> {
        let Lit::Int(int_lit) = self else { return None };
        Some(int_lit.base10_digits())
    }
}
