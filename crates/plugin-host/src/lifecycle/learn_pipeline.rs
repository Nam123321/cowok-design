use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LearnCardCategory {
    Rule,
    Pattern,
    AntiPattern,
    Lesson,
    Flow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardStatus {
    Proposed,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeCard {
    pub id: String,
    pub category: LearnCardCategory,
    pub title: String,
    pub content: String,
    pub status: CardStatus,
    pub source_evidence: String,
    pub created_at: i64,
    pub accepted_at: Option<i64>,
}

#[derive(Debug, Default)]
pub struct LearnPipeline {
    cards: Vec<KnowledgeCard>,
}

impl LearnPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a proposed card from gate failure history or user feedback
    pub fn propose_card(
        &mut self,
        id: &str,
        category: LearnCardCategory,
        title: &str,
        content: &str,
        source_evidence: &str,
    ) {
        self.cards.push(KnowledgeCard {
            id: id.to_string(),
            category,
            title: title.to_string(),
            content: content.to_string(),
            status: CardStatus::Proposed,
            source_evidence: source_evidence.to_string(),
            created_at: chrono::Utc::now().timestamp(),
            accepted_at: None,
        });
    }

    /// Human safety valve: accept card
    pub fn accept_card(&mut self, id: &str) -> bool {
        if let Some(card) = self.cards.iter_mut().find(|c| c.id == id) {
            card.status = CardStatus::Accepted;
            card.accepted_at = Some(chrono::Utc::now().timestamp());
            true
        } else {
            false
        }
    }

    /// Reject card
    pub fn reject_card(&mut self, id: &str) -> bool {
        if let Some(card) = self.cards.iter_mut().find(|c| c.id == id) {
            card.status = CardStatus::Rejected;
            true
        } else {
            false
        }
    }

    /// Returns all knowledge cards
    pub fn list_all_cards(&self) -> &[KnowledgeCard] {
        &self.cards
    }

    /// EC-LEARN-001 (Zero Unapproved Poisoning):
    /// Strictly filters and returns ONLY cards where status == CardStatus::Accepted.
    /// Proposed cards are NEVER injected into LLM system prompts without explicit human approval.
    pub fn get_active_prompt_injections(&self) -> Vec<&KnowledgeCard> {
        self.cards
            .iter()
            .filter(|c| c.status == CardStatus::Accepted)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_unapproved_poisoning_guard() {
        let mut pipeline = LearnPipeline::new();

        // 1. Propose card from gate failure
        pipeline.propose_card(
            "card-01",
            LearnCardCategory::AntiPattern,
            "No Purple Gradients",
            "Do not use #6366f1 as default gradient.",
            "gate-history.jsonl:line 42",
        );

        // Before human approval: 0 active injections!
        assert!(pipeline.get_active_prompt_injections().is_empty());
        assert_eq!(pipeline.list_all_cards().len(), 1);
        assert_eq!(pipeline.list_all_cards()[0].status, CardStatus::Proposed);

        // 2. Human reviews and accepts
        assert!(pipeline.accept_card("card-01"));
        assert_eq!(pipeline.list_all_cards()[0].status, CardStatus::Accepted);

        // After human approval: card is injected!
        let active = pipeline.get_active_prompt_injections();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "card-01");
        assert_eq!(active[0].title, "No Purple Gradients");
    }

    #[test]
    fn test_rejected_cards_never_injected() {
        let mut pipeline = LearnPipeline::new();
        pipeline.propose_card(
            "card-02",
            LearnCardCategory::Rule,
            "Dubious Rule",
            "Use red background everywhere",
            "unverified suggestion",
        );

        pipeline.reject_card("card-02");
        assert!(pipeline.get_active_prompt_injections().is_empty());
    }
}
