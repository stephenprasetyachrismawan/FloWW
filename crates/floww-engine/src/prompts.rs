//! Prompts for each AI step.
//!
//! Order inside every prompt (cache-friendly, and important things away from
//! the middle): stable team rules → confirmed rules/decisions → facts → the task.

use crate::model::{Criterion, GrillAnswer, Prd};

pub fn language_name(code: &str) -> &'static str {
    match code {
        "en" => "English",
        _ => "Bahasa Indonesia",
    }
}

pub struct Pack<'a> {
    pub language: &'a str,
    pub skills: &'a [String],
    pub rules: &'a [String],
    pub decisions: &'a [String],
    pub facts: &'a [String],
    pub gaps: &'a [String],
    /// Rough token budget (4 chars per token). Facts are cut first, then decisions.
    pub budget_tokens: usize,
}

impl Pack<'_> {
    /// The shared head of every prompt, trimmed to the budget.
    fn head(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("You are working inside FloWW for an Odoo project. Write anything a person reads in {}.\n", language_name(self.language)));
        if !self.skills.is_empty() {
            s.push_str(&format!(
                "Use these Claude Code skills for this step: {}.\n",
                self.skills.join(", ")
            ));
        }
        let budget = self.budget_tokens.saturating_mul(4);
        let add_section = |title: &str, items: &[String], s: &mut String| {
            if items.is_empty() {
                return;
            }
            let mut block = format!("\n## {title}\n");
            for it in items {
                let line = format!("- {it}\n");
                if s.len() + block.len() + line.len() > budget {
                    block.push_str("- (more items left out to stay within the context budget)\n");
                    break;
                }
                block.push_str(&line);
            }
            s.push_str(&block);
        };
        add_section(
            "Business rules (confirmed, must keep holding)",
            self.rules,
            &mut s,
        );
        add_section("Earlier decisions by the team", self.decisions, &mut s);
        add_section("Facts from the code and database", self.facts, &mut s);
        add_section(
            "Topics the person is not sure about (explain briefly before asking)",
            self.gaps,
            &mut s,
        );
        s
    }
}

fn request_block(prd: &Prd) -> String {
    format!(
        "\n## Request {id}\nTitle: {title}\nKind: {kind:?}\nAsked by: {rep}\n\n{desc}\n",
        id = prd.id,
        title = prd.title,
        kind = prd.kind,
        rep = if prd.reporter.is_empty() {
            "-"
        } else {
            &prd.reporter
        },
        desc = prd.description
    )
}

pub fn context(prd: &Prd, pack: &Pack, base_ref: &str) -> String {
    format!(
        "{head}{req}\n## Task\nYou are in a read-only checkout of `{base_ref}`. Find the code this request is about.\nUse Grep, Glob and Read. Do not edit files.\nReply with a short summary and then a fenced json block:\n```json\n{{\"summary\": \"2-4 sentences on how it works today\", \"modules\": [\"odoo_module\"], \"files\": [\"path/to/file.py\"], \"similar_prd\": null}}\n```\n",
        head = pack.head(),
        req = request_block(prd),
    )
}

const GRILL_FORMAT: &str = "Reply with only a fenced json block, one of:\n```json\n{\"done\": false, \"question\": {\"topic\": \"Data model\", \"text\": \"The question\", \"explain\": null, \"options\": [{\"label\": \"Option\", \"why\": \"one sentence\", \"recommended\": true}, {\"label\": \"Other option\", \"why\": \"one sentence\", \"recommended\": false}]}}\n```\nor, when you have enough to write a good PRD:\n```json\n{\"done\": true}\n```\nRules: one question at a time; 2-4 options; exactly one recommended; set \"explain\" to a 1-2 sentence explanation only when the topic is in the not-sure list.";

pub fn grill_start(prd: &Prd, pack: &Pack, summary: &str) -> String {
    format!(
        "{head}{req}\n## What the code does today\n{summary}\n\n## Task\nInterview the person (grill-me) to settle what the PRD needs: scope, data model, edge cases, migration, users. Do not ask what the code or the decisions above already answer.\n{GRILL_FORMAT}\n",
        head = pack.head(),
        req = request_block(prd),
    )
}

pub fn grill_answer(answer: &str, own: bool) -> String {
    let who = if own {
        "The person wrote their own answer"
    } else {
        "The person chose"
    };
    format!("{who}: \"{answer}\".\nAsk the next question, or finish.\n{GRILL_FORMAT}\n")
}

pub fn prd_write(answers: &[GrillAnswer]) -> String {
    let mut qa = String::new();
    for a in answers {
        qa.push_str(&format!("- {} → {}\n", a.question.text, a.answer));
    }
    format!(
        "Write the PRD now from the request, the code you read, and these answers:\n{qa}\nFirst the PRD in markdown with these sections: Masalah/Problem, Tujuan/Goal and non-goal, Siapa/Users, Kapan dianggap selesai/Acceptance criteria (Given/When/Then), Perubahan teknis/Technical changes (models, fields, views, security), Migration, Modul terdampak/Affected modules, Risiko/Risks and edge cases, Tes/Test plan.\nThen a fenced json block:\n```json\n{{\"criteria\": [{{\"given\": \"\", \"when\": \"\", \"then\": \"\"}}], \"tests\": [{{\"kind\": \"Normal|Edge\", \"title\": \"\", \"steps\": \"\", \"data\": \"\", \"expected\": \"\"}}], \"rules\": [{{\"id\": \"kebab-case-id\", \"title\": \"business rule in one sentence\", \"why\": \"\", \"enforced_in\": \"model.method\"}}], \"modules\": [\"odoo_module\"]}}\n```\nInclude at least one Normal and two Edge tests. Only list rules the business actually requires.\n"
    )
}

pub fn develop(
    prd: &Prd,
    pack: &Pack,
    row: usize,
    criterion: &Criterion,
    test_command: &str,
    extra: &str,
) -> String {
    let total = prd.criteria.len();
    format!(
        "{head}{req}\n## PRD\n{prd_md}\n\n## Task: acceptance criterion {n} of {total}\nGiven {g}\nWhen {w}\nThen {t}\n{extra}\nSteps:\n1. Read `.floww-progress.md` for what earlier sessions did.\n2. Quote the business rules and decisions above that apply to this criterion (or say none apply).\n3. Write a failing test for this criterion in the module's tests, run it, confirm it fails.\n4. Change the code until the test passes. Use only fields and methods listed in the facts or that you verified in the code.\n5. Run: `{test_command}`\n6. Commit locally: `git add -A && git commit -m \"wip({id}): criterion {n}\"`.\n7. Append to `.floww-progress.md` what you did and anything the next session must know.\nWork only on this criterion. Finish with one sentence saying whether the tests pass.\n",
        head = pack.head(),
        req = request_block(prd),
        prd_md = prd.prd_markdown.as_deref().unwrap_or(""),
        n = row + 1,
        g = criterion.given,
        w = criterion.when,
        t = criterion.then,
        id = prd.id,
    )
}

pub fn commit_message(prd: &Prd, stat: &str, diff: &str, format_hint: &str) -> String {
    format!(
        "Write the commit message for this change.\nFormat: {format_hint}. First line under 72 characters. Then a blank line and 1-4 lines on why. End with `PRD: {id}`.\nRequest: {title}\n\nFiles:\n{stat}\n\nDiff:\n{diff}\n\nReply with only a fenced json block: ```json\n{{\"message\": \"...\", \"pr_title\": \"...\", \"pr_body\": \"...\"}}\n```\n",
        id = prd.id,
        title = prd.title,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_respects_budget_and_keeps_rules_first() {
        let rules = vec!["Qty already invoiced is never invoiced again".to_string()];
        let facts: Vec<String> = (0..500)
            .map(|i| format!("fact number {i} with some padding text"))
            .collect();
        let pack = Pack {
            language: "id",
            skills: &[],
            rules: &rules,
            decisions: &[],
            facts: &facts,
            gaps: &[],
            budget_tokens: 300,
        };
        let head = pack.head();
        assert!(head.len() < 300 * 4 + 200, "{}", head.len());
        let r = head.find("never invoiced").unwrap();
        let f = head.find("fact number 0").unwrap();
        assert!(r < f);
        assert!(head.contains("left out"));
    }
}
