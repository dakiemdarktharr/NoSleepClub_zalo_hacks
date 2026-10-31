use crate::{BrokerError, Result};

pub(crate) fn calculate(text: &str) -> Result<f64> {
    if text.is_empty() || text.len() > 256 || !text.is_ascii() {
        return Err(BrokerError::Arithmetic);
    }
    let mut parser = Parser {
        input: text.as_bytes(),
        position: 0,
        depth: 0,
    };
    let value = parser.sum()?;
    parser.whitespace();
    if parser.position != parser.input.len() {
        return Err(BrokerError::Arithmetic);
    }
    finite(value)
}

fn finite(n: f64) -> Result<f64> {
    if n.is_finite() && n.abs() <= 1e100 {
        Ok(n)
    } else {
        Err(BrokerError::Arithmetic)
    }
}

struct Parser<'a> {
    input: &'a [u8],
    position: usize,
    depth: usize,
}
impl Parser<'_> {
    fn whitespace(&mut self) {
        while self
            .input
            .get(self.position)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.position += 1;
        }
    }
    fn take(&mut self, c: u8) -> bool {
        self.whitespace();
        if self.input.get(self.position) == Some(&c) {
            self.position += 1;
            true
        } else {
            false
        }
    }
    fn sum(&mut self) -> Result<f64> {
        let mut value = self.product()?;
        loop {
            if self.take(b'+') {
                value = finite(value + self.product()?)?;
            } else if self.take(b'-') {
                value = finite(value - self.product()?)?;
            } else {
                return Ok(value);
            }
        }
    }
    fn product(&mut self) -> Result<f64> {
        let mut value = self.atom()?;
        loop {
            if self.take(b'*') {
                value = finite(value * self.atom()?)?;
            } else if self.take(b'/') {
                let divisor = self.atom()?;
                if divisor == 0.0 {
                    return Err(BrokerError::Arithmetic);
                }
                value = finite(value / divisor)?;
            } else {
                return Ok(value);
            }
        }
    }
    fn atom(&mut self) -> Result<f64> {
        self.depth += 1;
        if self.depth > 16 {
            return Err(BrokerError::Arithmetic);
        }
        let mut value = if self.take(b'+') {
            self.atom()?
        } else if self.take(b'-') {
            -self.atom()?
        } else if self.take(b'(') {
            let n = self.sum()?;
            if !self.take(b')') {
                return Err(BrokerError::Arithmetic);
            }
            n
        } else {
            self.number()?
        };
        if self.take(b'%') {
            value /= 100.0;
        }
        self.depth -= 1;
        finite(value)
    }
    fn number(&mut self) -> Result<f64> {
        self.whitespace();
        let start = self.position;
        while self
            .input
            .get(self.position)
            .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
        {
            self.position += 1;
        }
        if start == self.position {
            return Err(BrokerError::Arithmetic);
        }
        let value = std::str::from_utf8(&self.input[start..self.position])
            .map_err(|_| BrokerError::Arithmetic)?
            .parse::<f64>()
            .map_err(|_| BrokerError::Arithmetic)?;
        finite(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precedence_unary_and_percent() {
        assert_eq!(calculate("-(2+3)*4 + 1000*15%").unwrap(), 130.0);
    }
    #[test]
    fn rejects_code_nonfinite_and_excessive_depth() {
        for text in [
            "1/0",
            "1e999",
            "NaN",
            "2; system(1)",
            "((((((((((((((((((1))))))))))))))))))",
            "1..2",
            "",
            "2**3",
            "1,000",
        ] {
            assert!(calculate(text).is_err(), "{text}");
        }
    }
}
