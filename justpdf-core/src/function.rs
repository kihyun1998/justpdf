//! PDF Function evaluator.
//!
//! Supports all 4 PDF function types:
//! - Type 0: Sampled function (lookup table)
//! - Type 2: Exponential interpolation
//! - Type 3: Stitching (piecewise)
//! - Type 4: PostScript calculator
//!
//! Reference: PDF 2.0 spec, section 7.10

use crate::object::{PdfDict, PdfObject};

/// A resolved PDF function that can be evaluated.
#[derive(Debug, Clone)]
pub enum PdfFunction {
    /// Type 2: Exponential interpolation.
    Exponential {
        domain: Vec<(f64, f64)>,
        range: Vec<(f64, f64)>,
        c0: Vec<f64>,
        c1: Vec<f64>,
        n: f64,
    },
    /// Type 3: Stitching of sub-functions.
    Stitching {
        domain: Vec<(f64, f64)>,
        range: Vec<(f64, f64)>,
        functions: Vec<PdfFunction>,
        bounds: Vec<f64>,
        encode: Vec<f64>,
    },
    /// Type 4: PostScript calculator.
    PostScript {
        domain: Vec<(f64, f64)>,
        range: Vec<(f64, f64)>,
        ops: Vec<PsOp>,
    },
}

/// PostScript calculator operations.
#[derive(Debug, Clone)]
pub enum PsOp {
    // Operand
    Num(f64),
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Idiv,
    Mod,
    Neg,
    Abs,
    Ceiling,
    Floor,
    Round,
    Truncate,
    Sqrt,
    Exp,
    Ln,
    Log,
    Sin,
    Cos,
    Atan,
    // Comparison
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    // Boolean
    And,
    Or,
    Not,
    Xor,
    // Bitwise
    Bitshift,
    // Conditional
    If(Vec<PsOp>),
    IfElse(Vec<PsOp>, Vec<PsOp>),
    // Stack
    Dup,
    Exch,
    Pop,
    Copy,
    Index,
    Roll,
    // Conversion
    Cvi,
    Cvr,
    True,
    False,
}

impl PdfFunction {
    /// Parse a PDF function from a function dictionary/stream.
    pub fn parse(obj: &PdfObject) -> Option<Self> {
        let dict = match obj {
            PdfObject::Dict(d) => d,
            PdfObject::Stream { dict, .. } => dict,
            _ => return None,
        };

        let func_type = dict.get_i64(b"FunctionType")?;
        let domain = parse_domain_range(dict, b"Domain");
        let range = parse_domain_range(dict, b"Range");

        match func_type {
            2 => Self::parse_exponential(dict, domain, range),
            3 => Self::parse_stitching(dict, domain, range),
            4 => Self::parse_postscript(obj, domain, range),
            _ => None,
        }
    }

    fn parse_exponential(
        dict: &PdfDict,
        domain: Vec<(f64, f64)>,
        range: Vec<(f64, f64)>,
    ) -> Option<Self> {
        let c0 = dict
            .get_array(b"C0")
            .map(|a| a.iter().filter_map(|o| o.as_f64()).collect())
            .unwrap_or_else(|| vec![0.0]);
        let c1 = dict
            .get_array(b"C1")
            .map(|a| a.iter().filter_map(|o| o.as_f64()).collect())
            .unwrap_or_else(|| vec![1.0]);
        let n = dict
            .get(b"N")
            .and_then(|o| o.as_f64())
            .unwrap_or(1.0);

        Some(PdfFunction::Exponential {
            domain,
            range,
            c0,
            c1,
            n,
        })
    }

    fn parse_stitching(
        dict: &PdfDict,
        domain: Vec<(f64, f64)>,
        range: Vec<(f64, f64)>,
    ) -> Option<Self> {
        let bounds = dict
            .get_array(b"Bounds")
            .map(|a| a.iter().filter_map(|o| o.as_f64()).collect())
            .unwrap_or_default();
        let encode = dict
            .get_array(b"Encode")
            .map(|a| a.iter().filter_map(|o| o.as_f64()).collect())
            .unwrap_or_default();

        let functions: Vec<PdfFunction> = dict
            .get_array(b"Functions")
            .map(|arr| arr.iter().filter_map(|o| PdfFunction::parse(o)).collect())
            .unwrap_or_default();

        Some(PdfFunction::Stitching {
            domain,
            range,
            functions,
            bounds,
            encode,
        })
    }

    fn parse_postscript(
        obj: &PdfObject,
        domain: Vec<(f64, f64)>,
        range: Vec<(f64, f64)>,
    ) -> Option<Self> {
        let stream_data = match obj {
            PdfObject::Stream { data, .. } => data,
            _ => return None,
        };

        let code = std::str::from_utf8(stream_data).ok()?;
        let ops = parse_ps_code(code)?;

        Some(PdfFunction::PostScript {
            domain,
            range,
            ops,
        })
    }

    /// Evaluate the function with given input values.
    /// Returns output values.
    pub fn evaluate(&self, input: &[f64]) -> Vec<f64> {
        match self {
            PdfFunction::Exponential {
                domain,
                range,
                c0,
                c1,
                n,
            } => {
                let x = clamp_input(input.first().copied().unwrap_or(0.0), domain);
                let out_len = c0.len().max(c1.len());
                let mut result = Vec::with_capacity(out_len);
                for i in 0..out_len {
                    let a = c0.get(i).copied().unwrap_or(0.0);
                    let b = c1.get(i).copied().unwrap_or(1.0);
                    let val = a + x.powf(*n) * (b - a);
                    result.push(clamp_output(val, range, i));
                }
                result
            }
            PdfFunction::Stitching {
                domain,
                range,
                functions,
                bounds,
                encode,
            } => {
                if functions.is_empty() {
                    return vec![0.0];
                }
                let x = clamp_input(input.first().copied().unwrap_or(0.0), domain);

                // Find which sub-function to use
                let mut idx = 0;
                for (i, &b) in bounds.iter().enumerate() {
                    if x < b {
                        idx = i;
                        break;
                    }
                    idx = i + 1;
                }
                idx = idx.min(functions.len() - 1);

                // Encode x into sub-function's domain
                let sub_domain_start = bounds.get(idx.wrapping_sub(1)).copied().unwrap_or_else(|| {
                    domain.first().map(|d| d.0).unwrap_or(0.0)
                });
                let sub_domain_end = bounds.get(idx).copied().unwrap_or_else(|| {
                    domain.first().map(|d| d.1).unwrap_or(1.0)
                });

                let enc_start = encode.get(idx * 2).copied().unwrap_or(0.0);
                let enc_end = encode.get(idx * 2 + 1).copied().unwrap_or(1.0);

                let denom = sub_domain_end - sub_domain_start;
                let encoded = if denom.abs() > 1e-10 {
                    enc_start + (x - sub_domain_start) / denom * (enc_end - enc_start)
                } else {
                    enc_start
                };

                let result = functions[idx].evaluate(&[encoded]);
                result
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| clamp_output(v, range, i))
                    .collect()
            }
            PdfFunction::PostScript {
                domain,
                range,
                ops,
            } => {
                let mut stack: Vec<f64> = Vec::new();
                // Push clamped inputs onto stack
                for (i, &val) in input.iter().enumerate() {
                    stack.push(clamp_input(val, &domain[i..i + 1].iter().copied().collect::<Vec<_>>()));
                }

                execute_ps_ops(&mut stack, ops);

                // Clamp outputs
                stack
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| clamp_output(v, range, i))
                    .collect()
            }
        }
    }
}

fn clamp_input(x: f64, domain: &[(f64, f64)]) -> f64 {
    if let Some(&(lo, hi)) = domain.first() {
        x.clamp(lo, hi)
    } else {
        x.clamp(0.0, 1.0)
    }
}

fn clamp_output(val: f64, range: &[(f64, f64)], idx: usize) -> f64 {
    if let Some(&(lo, hi)) = range.get(idx) {
        val.clamp(lo, hi)
    } else {
        val
    }
}

fn parse_domain_range(dict: &PdfDict, key: &[u8]) -> Vec<(f64, f64)> {
    dict.get_array(key)
        .map(|arr| {
            arr.chunks(2)
                .map(|pair| {
                    let lo = pair.first().and_then(|o| o.as_f64()).unwrap_or(0.0);
                    let hi = pair.get(1).and_then(|o| o.as_f64()).unwrap_or(1.0);
                    (lo, hi)
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// PostScript calculator parser
// ---------------------------------------------------------------------------

fn parse_ps_code(code: &str) -> Option<Vec<PsOp>> {
    // Strip outer braces { ... }
    let code = code.trim();
    let code = if code.starts_with('{') && code.ends_with('}') {
        &code[1..code.len() - 1]
    } else {
        code
    };

    parse_ps_block(code)
}

fn parse_ps_block(code: &str) -> Option<Vec<PsOp>> {
    let mut ops = Vec::new();
    let tokens = tokenize_ps(code);
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];
        i += 1;

        match token.as_str() {
            "{" => {
                // Find matching closing brace
                let (block, end) = extract_block(&tokens, i)?;
                i = end;

                // Check if followed by "if" or "ifelse"
                if i < tokens.len() && tokens[i] == "if" {
                    i += 1;
                    let block_ops = parse_ps_block(&block)?;
                    ops.push(PsOp::If(block_ops));
                } else if i + 1 < tokens.len() && tokens[i] == "{" {
                    // Could be: { block1 } { block2 } ifelse
                    let (block2, end2) = extract_block(&tokens, i + 1)?;
                    if end2 < tokens.len() && tokens[end2] == "ifelse" {
                        let block1_ops = parse_ps_block(&block)?;
                        let block2_ops = parse_ps_block(&block2)?;
                        ops.push(PsOp::IfElse(block1_ops, block2_ops));
                        i = end2 + 1;
                    } else {
                        // Just a block — push ops
                        let block_ops = parse_ps_block(&block)?;
                        ops.extend(block_ops);
                    }
                } else {
                    let block_ops = parse_ps_block(&block)?;
                    ops.extend(block_ops);
                }
            }
            "add" => ops.push(PsOp::Add),
            "sub" => ops.push(PsOp::Sub),
            "mul" => ops.push(PsOp::Mul),
            "div" => ops.push(PsOp::Div),
            "idiv" => ops.push(PsOp::Idiv),
            "mod" => ops.push(PsOp::Mod),
            "neg" => ops.push(PsOp::Neg),
            "abs" => ops.push(PsOp::Abs),
            "ceiling" => ops.push(PsOp::Ceiling),
            "floor" => ops.push(PsOp::Floor),
            "round" => ops.push(PsOp::Round),
            "truncate" => ops.push(PsOp::Truncate),
            "sqrt" => ops.push(PsOp::Sqrt),
            "exp" => ops.push(PsOp::Exp),
            "ln" => ops.push(PsOp::Ln),
            "log" => ops.push(PsOp::Log),
            "sin" => ops.push(PsOp::Sin),
            "cos" => ops.push(PsOp::Cos),
            "atan" => ops.push(PsOp::Atan),
            "eq" => ops.push(PsOp::Eq),
            "ne" => ops.push(PsOp::Ne),
            "gt" => ops.push(PsOp::Gt),
            "ge" => ops.push(PsOp::Ge),
            "lt" => ops.push(PsOp::Lt),
            "le" => ops.push(PsOp::Le),
            "and" => ops.push(PsOp::And),
            "or" => ops.push(PsOp::Or),
            "not" => ops.push(PsOp::Not),
            "xor" => ops.push(PsOp::Xor),
            "bitshift" => ops.push(PsOp::Bitshift),
            "dup" => ops.push(PsOp::Dup),
            "exch" => ops.push(PsOp::Exch),
            "pop" => ops.push(PsOp::Pop),
            "copy" => ops.push(PsOp::Copy),
            "index" => ops.push(PsOp::Index),
            "roll" => ops.push(PsOp::Roll),
            "cvi" => ops.push(PsOp::Cvi),
            "cvr" => ops.push(PsOp::Cvr),
            "true" => ops.push(PsOp::True),
            "false" => ops.push(PsOp::False),
            "if" | "ifelse" => {} // handled above with blocks
            _ => {
                // Try to parse as number
                if let Ok(n) = token.parse::<f64>() {
                    ops.push(PsOp::Num(n));
                }
                // else: unknown token, skip
            }
        }
    }

    Some(ops)
}

fn tokenize_ps(code: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in code.chars() {
        match ch {
            '{' | '}' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(ch.to_string());
            }
            ' ' | '\t' | '\n' | '\r' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Extract a brace-delimited block from tokens starting at position `start`.
/// Returns the block content as a string and the position after the closing brace.
fn extract_block(tokens: &[String], start: usize) -> Option<(String, usize)> {
    let mut depth = 1;
    let mut i = start;
    let mut parts = Vec::new();

    while i < tokens.len() && depth > 0 {
        if tokens[i] == "{" {
            depth += 1;
            parts.push(tokens[i].clone());
        } else if tokens[i] == "}" {
            depth -= 1;
            if depth > 0 {
                parts.push(tokens[i].clone());
            }
        } else {
            parts.push(tokens[i].clone());
        }
        i += 1;
    }

    Some((parts.join(" "), i))
}

// ---------------------------------------------------------------------------
// PostScript calculator executor
// ---------------------------------------------------------------------------

fn execute_ps_ops(stack: &mut Vec<f64>, ops: &[PsOp]) {
    for op in ops {
        match op {
            PsOp::Num(n) => stack.push(*n),
            PsOp::True => stack.push(1.0),
            PsOp::False => stack.push(0.0),

            // Arithmetic
            PsOp::Add => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(a + b);
                }
            }
            PsOp::Sub => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(a - b);
                }
            }
            PsOp::Mul => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(a * b);
                }
            }
            PsOp::Div => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(if b.abs() > 1e-20 { a / b } else { 0.0 });
                }
            }
            PsOp::Idiv => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    let bi = b as i64;
                    let ai = a as i64;
                    stack.push(if bi != 0 { (ai / bi) as f64 } else { 0.0 });
                }
            }
            PsOp::Mod => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    let bi = b as i64;
                    let ai = a as i64;
                    stack.push(if bi != 0 { (ai % bi) as f64 } else { 0.0 });
                }
            }
            PsOp::Neg => {
                if let Some(a) = stack.pop() {
                    stack.push(-a);
                }
            }
            PsOp::Abs => {
                if let Some(a) = stack.pop() {
                    stack.push(a.abs());
                }
            }
            PsOp::Ceiling => {
                if let Some(a) = stack.pop() {
                    stack.push(a.ceil());
                }
            }
            PsOp::Floor => {
                if let Some(a) = stack.pop() {
                    stack.push(a.floor());
                }
            }
            PsOp::Round => {
                if let Some(a) = stack.pop() {
                    stack.push(a.round());
                }
            }
            PsOp::Truncate => {
                if let Some(a) = stack.pop() {
                    stack.push(a.trunc());
                }
            }
            PsOp::Sqrt => {
                if let Some(a) = stack.pop() {
                    stack.push(if a >= 0.0 { a.sqrt() } else { 0.0 });
                }
            }
            PsOp::Exp => {
                if let (Some(e), Some(base)) = (stack.pop(), stack.pop()) {
                    stack.push(base.powf(e));
                }
            }
            PsOp::Ln => {
                if let Some(a) = stack.pop() {
                    stack.push(if a > 0.0 { a.ln() } else { 0.0 });
                }
            }
            PsOp::Log => {
                if let Some(a) = stack.pop() {
                    stack.push(if a > 0.0 { a.log10() } else { 0.0 });
                }
            }
            PsOp::Sin => {
                if let Some(a) = stack.pop() {
                    stack.push(a.to_radians().sin());
                }
            }
            PsOp::Cos => {
                if let Some(a) = stack.pop() {
                    stack.push(a.to_radians().cos());
                }
            }
            PsOp::Atan => {
                if let (Some(x), Some(y)) = (stack.pop(), stack.pop()) {
                    stack.push(y.atan2(x).to_degrees());
                }
            }

            // Comparison (push 1.0 for true, 0.0 for false)
            PsOp::Eq => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(if (a - b).abs() < 1e-10 { 1.0 } else { 0.0 });
                }
            }
            PsOp::Ne => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(if (a - b).abs() >= 1e-10 { 1.0 } else { 0.0 });
                }
            }
            PsOp::Gt => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(if a > b { 1.0 } else { 0.0 });
                }
            }
            PsOp::Ge => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(if a >= b { 1.0 } else { 0.0 });
                }
            }
            PsOp::Lt => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(if a < b { 1.0 } else { 0.0 });
                }
            }
            PsOp::Le => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(if a <= b { 1.0 } else { 0.0 });
                }
            }

            // Boolean / bitwise
            PsOp::And => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(((a as i64) & (b as i64)) as f64);
                }
            }
            PsOp::Or => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(((a as i64) | (b as i64)) as f64);
                }
            }
            PsOp::Not => {
                if let Some(a) = stack.pop() {
                    stack.push(if a == 0.0 { 1.0 } else { 0.0 });
                }
            }
            PsOp::Xor => {
                if let (Some(b), Some(a)) = (stack.pop(), stack.pop()) {
                    stack.push(((a as i64) ^ (b as i64)) as f64);
                }
            }
            PsOp::Bitshift => {
                if let (Some(shift), Some(val)) = (stack.pop(), stack.pop()) {
                    let v = val as i64;
                    let s = shift as i32;
                    let result = if s > 0 { v << s } else { v >> (-s) };
                    stack.push(result as f64);
                }
            }

            // Conditional
            PsOp::If(block) => {
                if let Some(cond) = stack.pop() {
                    if cond != 0.0 {
                        execute_ps_ops(stack, block);
                    }
                }
            }
            PsOp::IfElse(true_block, false_block) => {
                if let Some(cond) = stack.pop() {
                    if cond != 0.0 {
                        execute_ps_ops(stack, true_block);
                    } else {
                        execute_ps_ops(stack, false_block);
                    }
                }
            }

            // Stack manipulation
            PsOp::Dup => {
                if let Some(&top) = stack.last() {
                    stack.push(top);
                }
            }
            PsOp::Exch => {
                let len = stack.len();
                if len >= 2 {
                    stack.swap(len - 1, len - 2);
                }
            }
            PsOp::Pop => {
                stack.pop();
            }
            PsOp::Copy => {
                if let Some(n) = stack.pop() {
                    let n = n as usize;
                    let len = stack.len();
                    if n <= len {
                        let start = len - n;
                        let copied: Vec<f64> = stack[start..].to_vec();
                        stack.extend_from_slice(&copied);
                    }
                }
            }
            PsOp::Index => {
                if let Some(n) = stack.pop() {
                    let n = n as usize;
                    let len = stack.len();
                    if n < len {
                        let val = stack[len - 1 - n];
                        stack.push(val);
                    }
                }
            }
            PsOp::Roll => {
                if let (Some(j), Some(n)) = (stack.pop(), stack.pop()) {
                    let n = n as usize;
                    let j = j as i64;
                    let len = stack.len();
                    if n > 0 && n <= len {
                        let start = len - n;
                        let j = ((j % n as i64) + n as i64) as usize % n;
                        let split = n - j;
                        let mut temp: Vec<f64> = stack[start..].to_vec();
                        temp.rotate_left(split);
                        stack[start..].copy_from_slice(&temp);
                    }
                }
            }

            // Conversion
            PsOp::Cvi => {
                if let Some(a) = stack.pop() {
                    stack.push((a as i64) as f64);
                }
            }
            PsOp::Cvr => {
                // Already f64, no-op
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ps_simple_add() {
        let func = PdfFunction::PostScript {
            domain: vec![(0.0, 1.0)],
            range: vec![(0.0, 2.0)],
            ops: vec![PsOp::Num(1.0), PsOp::Add],
        };
        let result = func.evaluate(&[0.5]);
        assert!((result[0] - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_ps_mul_sub() {
        let func = PdfFunction::PostScript {
            domain: vec![(0.0, 1.0)],
            range: vec![(0.0, 1.0)],
            ops: vec![PsOp::Num(2.0), PsOp::Mul, PsOp::Num(0.5), PsOp::Sub],
        };
        // input 0.75 → 0.75*2 - 0.5 = 1.0
        let result = func.evaluate(&[0.75]);
        assert!((result[0] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_ps_dup() {
        let func = PdfFunction::PostScript {
            domain: vec![(0.0, 1.0)],
            range: vec![(0.0, 1.0), (0.0, 1.0)],
            ops: vec![PsOp::Dup],
        };
        let result = func.evaluate(&[0.7]);
        assert_eq!(result.len(), 2);
        assert!((result[0] - 0.7).abs() < 0.001);
        assert!((result[1] - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_ps_if() {
        // { dup 0.5 gt { 1.0 } { 0.0 } ifelse }
        let func = PdfFunction::PostScript {
            domain: vec![(0.0, 1.0)],
            range: vec![(0.0, 1.0)],
            ops: vec![
                PsOp::Dup,
                PsOp::Num(0.5),
                PsOp::Gt,
                PsOp::IfElse(vec![PsOp::Pop, PsOp::Num(1.0)], vec![PsOp::Pop, PsOp::Num(0.0)]),
            ],
        };
        assert!((func.evaluate(&[0.8])[0] - 1.0).abs() < 0.001);
        assert!((func.evaluate(&[0.3])[0] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_ps_neg_abs() {
        let func = PdfFunction::PostScript {
            domain: vec![(-1.0, 1.0)],
            range: vec![(0.0, 1.0)],
            ops: vec![PsOp::Neg, PsOp::Abs],
        };
        assert!((func.evaluate(&[-0.5])[0] - 0.5).abs() < 0.001);
        assert!((func.evaluate(&[0.3])[0] - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_exponential() {
        let func = PdfFunction::Exponential {
            domain: vec![(0.0, 1.0)],
            range: vec![(0.0, 1.0)],
            c0: vec![0.0],
            c1: vec![1.0],
            n: 2.0,
        };
        // f(0.5) = 0 + 0.5^2 * (1 - 0) = 0.25
        assert!((func.evaluate(&[0.5])[0] - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_parse_ps_code() {
        let code = "{ 1 add 2 mul }";
        let ops = parse_ps_code(code).unwrap();
        assert_eq!(ops.len(), 4); // Num(1), Add, Num(2), Mul
    }

    #[test]
    fn test_ps_trig() {
        let func = PdfFunction::PostScript {
            domain: vec![(0.0, 360.0)],
            range: vec![(-1.0, 1.0)],
            ops: vec![PsOp::Sin],
        };
        // sin(90°) = 1.0
        assert!((func.evaluate(&[90.0])[0] - 1.0).abs() < 0.001);
        // sin(0°) = 0.0
        assert!((func.evaluate(&[0.0])[0] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_ps_exch_roll() {
        let func = PdfFunction::PostScript {
            domain: vec![(0.0, 1.0), (0.0, 1.0)],
            range: vec![(0.0, 1.0), (0.0, 1.0)],
            ops: vec![PsOp::Exch],
        };
        let result = func.evaluate(&[0.2, 0.8]);
        assert!((result[0] - 0.8).abs() < 0.001);
        assert!((result[1] - 0.2).abs() < 0.001);
    }
}
