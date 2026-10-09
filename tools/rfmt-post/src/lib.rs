//! Post-rustfmt pass: split overflowing method chains outside-in, and long `matches!` one argument per line.
//!
//! rustfmt keeps `recv.a(..).b(` on one line and overflows the last call's arguments onto later lines. Here such a
//! chain is broken first, one link per line as rustfmt breaks chains, and then the last call's arguments go one per
//! line. Only whitespace changes, plus trailing commas and the braces around a closure body that is one expression.

use std::collections::HashMap;

use ra_ap_syntax::ast::{self, AstNode, HasArgList};
use ra_ap_syntax::{Edition, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken, T};

const WIDTH: usize = 120;
const MATCHES_WIDTH: usize = 72; // rustfmt's fn_call_width at max_width 120 (60%)
const INDENT: usize = 4;

#[derive(Default)]
pub struct Stats {
    pub chains: usize,
    pub too_wide: usize, // chains left as rustfmt wrote them: split, a line would pass WIDTH
    pub matches: usize,
}

/// Long `matches!` broken, then chains split, in one file of rustfmt output.
///
/// rustfmt keeps a `matches!` it cannot parse as written, and lays out what is around it by its width. So the
/// pipeline breaks them, runs rustfmt again, and only then splits chains; otherwise the next run's rustfmt would see
/// them broken where this one saw them whole, and lay out differently.
pub fn format(text: &str) -> Result<(String, Stats), String> {
    let mut doc = Doc::parse(text)?;
    let matches = break_matches(&mut doc);
    let mut broken = Doc::parse(&doc.render())?;
    let chains = split_chains(&mut broken);
    checked(&doc, broken.render()).map(|out| (out, Stats { chains: chains.0, too_wide: chains.1, matches }))
}

/// Long `matches!` broken, the first step of the pipeline.
pub fn matches_only(text: &str) -> Result<(String, usize), String> {
    let mut doc = Doc::parse(text)?;
    let count = break_matches(&mut doc);
    checked(&doc, doc.render()).map(|out| (out, count))
}

/// `out` if its tokens are those of `doc`, up to trailing commas and closure braces.
fn checked(
    doc: &Doc,
    out: String,
) -> Result<String, String> {
    if normalized(doc) != normalized(&Doc::parse(&out)?) {
        return Err("tokens changed".into());
    }
    Ok(out)
}

struct Tok {
    kind: SyntaxKind,
    text: String,
    start: usize,
    line: usize,
}

/// Non-whitespace tokens (comments included), the whitespace before each, and pending edits to both.
struct Doc {
    root: SyntaxNode,
    toks: Vec<Tok>,
    gaps: Vec<String>, // gaps[i] precedes toks[i]; the last one ends the file
    at: HashMap<usize, usize>,
    line_starts: Vec<usize>,
    first_on_line: Vec<usize>,
    edit: Edits,
}

#[derive(Clone)]
struct Edits {
    gap: Vec<Option<String>>,
    shift: Vec<isize>, // added to the indent of a gap that ends a line, unless `gap` replaces it
    drop: Vec<bool>,
}

impl Doc {
    fn parse(text: &str) -> Result<Doc, String> {
        let parse = SourceFile::parse(text, Edition::Edition2024);
        if let Some(error) = parse.errors().first() {
            return Err(format!("parse error: {error}"));
        }
        let root = parse.syntax_node();
        let line_starts: Vec<usize> = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
        let (mut toks, mut gaps, mut at) = (vec![], vec![String::new()], HashMap::new());
        for token in root.descendants_with_tokens().filter_map(|e| e.into_token()) {
            if token.kind() == SyntaxKind::WHITESPACE {
                gaps.last_mut().unwrap().push_str(token.text());
                continue;
            }
            let start = usize::from(token.text_range().start());
            at.insert(start, toks.len());
            let line = line_starts.partition_point(|&s| s <= start) - 1;
            toks.push(Tok { kind: token.kind(), text: token.text().to_string(), start, line });
            gaps.push(String::new());
        }
        let mut first_on_line = vec![usize::MAX; line_starts.len()];
        for (i, tok) in toks.iter().enumerate().rev() {
            first_on_line[tok.line] = i;
        }
        let edit = Edits { gap: vec![None; gaps.len()], shift: vec![0; gaps.len()], drop: vec![false; toks.len()] };
        Ok(Doc { root, toks, gaps, at, line_starts, first_on_line, edit })
    }

    fn idx(
        &self,
        token: &SyntaxToken,
    ) -> usize {
        self.at[&usize::from(token.text_range().start())]
    }

    fn first(
        &self,
        node: &SyntaxNode,
    ) -> usize {
        self.at[&usize::from(node.text_range().start())]
    }

    fn last(
        &self,
        node: &SyntaxNode,
    ) -> usize {
        let end = usize::from(node.text_range().end());
        self.toks.partition_point(|t| t.start < end) - 1
    }

    fn col(
        &self,
        i: usize,
    ) -> usize {
        self.toks[i].start - self.line_starts[self.toks[i].line]
    }

    /// The indent of the line toks[i] starts, if it is the first token there.
    fn indent(
        &self,
        i: usize,
    ) -> Option<usize> {
        let gap = &self.gaps[i];
        let line = self.toks[i].line;
        (self.first_on_line[line] == i && (i == 0 || gap.contains('\n')))
            .then(|| gap.len() - gap.rfind('\n').map_or(0, |p| p + 1))
    }

    fn has_comment(
        &self,
        from: usize,
        to: usize,
    ) -> bool {
        (from..=to).any(|i| self.toks[i].kind == SyntaxKind::COMMENT)
    }

    fn gap(
        &self,
        i: usize,
    ) -> String {
        if let Some(gap) = &self.edit.gap[i] {
            return gap.clone();
        }
        let gap = &self.gaps[i];
        match gap.rfind('\n') {
            Some(p) if self.edit.shift[i] != 0 => {
                let indent = (gap.len() - p - 1).checked_add_signed(self.edit.shift[i]).expect("negative indent");
                format!("{}{}", &gap[..=p], " ".repeat(indent))
            }
            _ => gap.clone(),
        }
    }

    fn render_range(
        &self,
        from: usize,
        to: usize,
    ) -> String {
        let mut out = String::new();
        for i in from..to {
            out += &self.gap(i);
            if i < self.toks.len() && !self.edit.drop[i] {
                out += &self.toks[i].text;
            }
        }
        out
    }

    fn render(&self) -> String {
        self.render_range(0, self.gaps.len())
    }

    /// Rendered lines from the one holding toks[from] to the one holding toks[to], and how many are too wide.
    fn too_wide(
        &self,
        mut from: usize,
        mut to: usize,
    ) -> usize {
        while from > 0 && !self.gap(from).contains('\n') {
            from -= 1;
        }
        while to + 1 < self.toks.len() && !self.gap(to + 1).contains('\n') {
            to += 1;
        }
        let text = self.render_range(from, to + 1);
        text.split('\n').skip(1).filter(|line| line.chars().count() > WIDTH).count()
    }

    fn set_gap(
        &mut self,
        i: usize,
        gap: String,
    ) {
        self.edit.gap[i] = Some(gap);
    }

    fn shift(
        &mut self,
        from: usize,
        to: usize,
        by: isize,
    ) {
        self.edit.shift[from..to].iter_mut().for_each(|s| *s += by);
    }
}

fn newline(indent: usize) -> String {
    format!("\n{}", " ".repeat(indent))
}

/// A chain whose last call's arguments overflow onto later lines, in rustfmt's layout.
struct Site {
    first: usize,       // the line's first token
    breaks: Vec<usize>, // the `.` of each link that goes on its own line
    lparen: usize,
    rparen: usize,
    commas: Vec<usize>,
    last_arg: usize, // its first token
    vertical: bool,
    joinable: bool,
    braces: Option<(usize, usize)>, // around a closure body that is one expression
}

impl Site {
    fn find(
        doc: &Doc,
        call: &ast::MethodCallExpr,
    ) -> Option<Site> {
        use SyntaxKind::*;
        let mut top = call.syntax().clone();
        while let Some(parent) = top.parent().filter(|p| p.kind() == TRY_EXPR) {
            top = parent;
        }
        let mut dots = vec![];
        let mut link: ast::Expr = call.clone().into();
        let root = loop {
            link = match link {
                ast::Expr::MethodCallExpr(e) => {
                    dots.push(doc.idx(&e.dot_token()?));
                    e.receiver()?
                }
                ast::Expr::FieldExpr(e) => {
                    dots.push(doc.idx(&e.dot_token()?));
                    e.expr()?
                }
                ast::Expr::AwaitExpr(e) => {
                    dots.push(doc.idx(&e.dot_token()?));
                    e.expr()?
                }
                ast::Expr::TryExpr(e) => e.expr()?,
                ast::Expr::IndexExpr(e) => e.base()?,
                root => break root,
            }
        };
        dots.reverse();

        // rustfmt keeps an expression with a comment between its operands as written, so its layout after a rerun
        // cannot be predicted.
        let verbatim = top
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| t.kind() == COMMENT && t.parent().is_some_and(|p| ast::Expr::can_cast(p.kind())));
        if verbatim {
            return None;
        }

        // Where the chain may stand; the statement holding it must start the chain's line.
        let mut context = top.clone();
        while let Some(parent) = context.parent().filter(|p| matches!(p.kind(), REF_EXPR | PREFIX_EXPR)) {
            context = parent;
        }
        let parent = context.parent()?;
        let anchor = match parent.kind() {
            LET_STMT => {
                let stmt = ast::LetStmt::cast(parent.clone())?;
                (stmt.let_else().is_none() && stmt.initializer()?.syntax() == &context).then_some(parent)?
            }
            BIN_EXPR => {
                let expr = ast::BinExpr::cast(parent.clone())?;
                let assigns = matches!(expr.op_kind()?, ast::BinaryOp::Assignment { .. });
                (assigns && expr.rhs()?.syntax() == &context).then_some(parent)?
            }
            EXPR_STMT | RETURN_EXPR | BREAK_EXPR | RECORD_EXPR_FIELD => parent,
            STMT_LIST | ARG_LIST => context,
            _ => return None,
        };
        let first = doc.first(&anchor);
        let start = doc.first(root.syntax());
        let line = doc.toks[start].line;
        let indent = doc.indent(first)?;

        let args = call.arg_list()?;
        let lparen = doc.idx(&args.l_paren_token()?);
        let rparen = doc.idx(&args.r_paren_token()?);
        let end = doc.toks[rparen].line;
        let after: Vec<usize> = (doc.last(&top) + 1..doc.toks.len()).take_while(|&i| doc.toks[i].line == end).collect();
        if doc.toks[first].line != line
            || doc.toks[lparen].line != line
            || end == line
            || (first..doc.toks.len()).take_while(|&i| doc.toks[i].line == line).any(|i| doc.has_comment(i, i))
            || !(after.is_empty() || after.len() == 1 && matches!(doc.toks[after[0]].kind, T![;] | T![,]))
        {
            return None;
        }

        // rustfmt keeps the first links on the root's line while the root is no wider than an indent; a chain is
        // two links or more left after that (`self.items.retain(..)` is one).
        let budget = INDENT as isize - (doc.col(start) - indent) as isize;
        let mut kept = 0;
        while kept < dots.len() && (doc.col(dots[kept]) - doc.col(start)) as isize <= budget {
            kept += 1;
        }
        if dots.len() - kept < 2 {
            return None;
        }

        let items: Vec<ast::Expr> = args.args().collect();
        let commas: Vec<usize> = args
            .syntax()
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == T![,])
            .map(|t| doc.idx(&t))
            .collect();
        let last = items.last()?;
        let last_arg = doc.first(last.syntax());
        let vertical = doc.toks[lparen + 1].line > line;
        let mut site = Site {
            first,
            breaks: dots[kept..].to_vec(),
            lparen,
            rparen,
            commas,
            last_arg,
            vertical,
            joinable: false,
            braces: None,
        };
        if vertical {
            site.joinable = items.len() == end - line - 1
                && items.iter().enumerate().all(|(k, item)| {
                    let (a, b) = (doc.first(item.syntax()), doc.last(item.syntax()));
                    doc.toks[a].line == line + 1 + k && doc.toks[b].line == line + 1 + k
                })
                && !doc.has_comment(lparen, rparen)
                && !(lparen..rparen).any(|i| doc.toks[i].text.contains('\n'));
            return Some(site);
        }
        if doc.toks[last_arg].line != line || site.commas.len() != items.len() - 1 || !doc.gaps[rparen].is_empty() {
            return None;
        }
        site.braces = bare_body(doc, last).filter(|&(open, close)| {
            doc.toks[open].line == line && doc.toks[open + 1].line == line + 1 && close + 1 == rparen
        });
        Some(site)
    }

    fn apply(
        &self,
        doc: &mut Doc,
    ) {
        let line_indent = doc.indent(self.first).unwrap() as isize + doc.edit.shift[self.first];
        let link = newline(line_indent as usize + INDENT);
        let item = newline(line_indent as usize + 2 * INDENT);
        for &dot in &self.breaks {
            doc.set_gap(dot, link.clone());
        }
        let (lparen, rparen) = (self.lparen, self.rparen);
        if self.vertical {
            if self.joinable {
                let saved = doc.edit.clone();
                doc.set_gap(lparen + 1, String::new());
                for &comma in &self.commas {
                    doc.set_gap(comma + 1, " ".into());
                }
                if self.commas.last() == Some(&(rparen - 1)) {
                    doc.edit.drop[rparen - 1] = true;
                }
                doc.set_gap(rparen, String::new());
                if doc.too_wide(lparen, lparen) == 0 {
                    return;
                }
                doc.edit = saved;
            }
            doc.shift(lparen + 1, rparen + 1, INDENT as isize);
            return;
        }
        doc.set_gap(lparen + 1, item.clone());
        for &comma in &self.commas {
            doc.set_gap(comma + 1, item.clone());
        }
        if let Some((open, close)) = self.braces {
            // The body's lines lose the braces' indent and gain the arguments'.
            doc.edit.drop[open] = true;
            doc.edit.drop[close] = true;
            doc.shift(open + 1, close, INDENT as isize);
            doc.set_gap(open + 1, String::new());
            doc.set_gap(close, ",".into());
            doc.set_gap(rparen, link);
        } else {
            doc.shift(self.last_arg + 1, rparen, 2 * INDENT as isize);
            doc.set_gap(rparen, format!(",{link}"));
        }
    }
}

/// The braces of a closure whose block body holds one expression and nothing else.
fn bare_body(
    doc: &Doc,
    arg: &ast::Expr,
) -> Option<(usize, usize)> {
    let ast::Expr::ClosureExpr(closure) = arg else { return None };
    let ast::Expr::BlockExpr(block) = closure.body()? else { return None };
    let stmts = block.stmt_list()?;
    let open = doc.idx(&stmts.l_curly_token()?);
    let close = doc.idx(&stmts.r_curly_token()?);
    let plain = closure.ret_type().is_none() && doc.first(block.syntax()) == open;
    let single = stmts.statements().next().is_none() && stmts.tail_expr().is_some();
    let attributed = stmts.tail_expr().is_some_and(|e| doc.toks[doc.first(e.syntax())].kind == T![#]);
    (plain && single && !attributed && !doc.has_comment(open, close)).then_some((open, close))
}

/// How many chains were split, and how many were left because a line would pass WIDTH.
fn split_chains(doc: &mut Doc) -> (usize, usize) {
    let calls: Vec<ast::MethodCallExpr> = doc.root.descendants().filter_map(ast::MethodCallExpr::cast).collect();
    let mut sites: Vec<Site> = calls.iter().filter_map(|call| Site::find(doc, call)).collect();
    sites.sort_by_key(|site| site.first); // outer sites first: an inner one indents from where the outer put it
    let (mut count, mut wide_left) = (0, 0);
    for site in sites {
        let saved = doc.edit.clone();
        let wide = doc.too_wide(site.first, site.rparen);
        site.apply(doc);
        if doc.too_wide(site.first, site.rparen) > wide {
            doc.edit = saved;
            wide_left += 1;
        } else {
            count += 1;
        }
    }
    (count, wide_left)
}

/// Each argument of a `matches!` longer than MATCHES_WIDTH on its own line; rustfmt keeps macro contents.
fn break_matches(doc: &mut Doc) -> usize {
    let (mut done_to, mut count) = (0, 0);
    let calls: Vec<ast::MacroCall> = doc.root.descendants().filter_map(ast::MacroCall::cast).collect();
    for call in calls {
        let Some(tree) = call.token_tree() else { continue };
        let (Some(open), Some(close)) = (tree.l_paren_token(), tree.r_paren_token()) else { continue };
        let (open, close) = (doc.idx(&open), doc.idx(&close));
        if call.path().is_none_or(|p| p.syntax().text() != "matches") || open < done_to || doc.has_comment(open, close)
        {
            continue;
        }
        let commas: Vec<usize> = tree
            .syntax()
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == T![,])
            .map(|t| doc.idx(&t))
            .collect();
        let Some(indent) = doc.indent(doc.first_on_line[doc.toks[open].line]) else { continue };
        let item = newline(indent + INDENT);
        let at = doc.gaps[open + 1].rfind('\n');
        if let Some(at) = at {
            // Already one argument per line, by rustfmt or an earlier run: only indented under this line.
            let by = (indent + INDENT) as isize - (doc.gaps[open + 1].len() - at - 1) as isize;
            doc.shift(open + 1, close, by);
            doc.set_gap(close, newline(indent));
            done_to = close;
            continue;
        }
        let trailing = commas.last() == Some(&(close - 1));
        let args = commas.len() + 1 - trailing as usize;
        // Its width with each argument on one line: one space wherever the source has whitespace.
        let flat: usize = (open + 1..close)
            .map(|i| doc.toks[i].text.chars().count() + (i > open + 1 && !doc.gaps[i].is_empty()) as usize)
            .sum();
        if args < 2 || "matches!()".len() + flat - trailing as usize <= MATCHES_WIDTH {
            continue;
        }
        for i in open + 2..close {
            if !doc.gaps[i].is_empty() {
                doc.set_gap(i, " ".into());
            }
        }
        doc.set_gap(open + 1, item.clone());
        for &comma in &commas {
            doc.set_gap(comma + 1, item.clone());
        }
        if trailing {
            doc.edit.drop[close - 1] = true;
        }
        doc.set_gap(close, newline(indent));
        done_to = close;
        count += 1;
    }
    count
}

/// Token texts, ignoring trailing commas and the braces of a closure body.
fn normalized(doc: &Doc) -> Vec<&str> {
    let toks = &doc.toks;
    let mut drop = vec![false; toks.len()];
    let mut open = vec![];
    for (i, tok) in toks.iter().enumerate() {
        let next = toks.get(i + 1).map(|t| t.kind);
        if tok.kind == T![,] && matches!(next, Some(T![')'] | T![']'] | T!['}'])) {
            drop[i] = true;
        }
        match tok.kind {
            T!['{'] => open.push((i, i > 0 && matches!(toks[i - 1].kind, T![|] | T![||]))),
            T!['}'] => {
                let (o, closure) = open.pop().unwrap();
                drop[o] |= closure;
                drop[i] |= closure;
            }
            _ => {}
        }
    }
    toks.iter().zip(drop).filter(|(_, d)| !d).map(|(t, _)| t.text.as_str()).collect()
}
