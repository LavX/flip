use rand::Rng;
use serde_json::{json, Map, Value};

use crate::models::Question;

/// Produces the answer payload for one already validated question.
///
/// Each call gets one 1-in-6000 chance to leave the ordinary path, unless
/// `force_edge` selects that path directly. The coin is fair, even when it is
/// being deliberately unhelpful.
pub fn answer(question: &Question, force_edge: bool, rng: &mut impl Rng) -> Value {
    let edge = force_edge || rng.gen_range(0..6000) == 0;

    match question {
        Question::Noul => {
            let noul = if edge {
                0.5
            } else if rng.gen_bool(0.5) {
                1.0
            } else {
                0.0
            };
            json!({"type": "noul", "noul": noul})
        }
        Question::Choice(criteria) => {
            let options: Vec<&String> = criteria.keys().collect();
            let option_count = options.len();
            let winner = rng.gen_range(0..option_count);
            let probabilities = choice_probabilities(&options, edge, winner);
            let top_probability = if edge && option_count > 1 {
                1.0 / option_count as f64
            } else {
                1.0
            };
            let confidence = choice_confidence(top_probability, option_count);

            json!({
                "type": "choice",
                "choice": options[winner],
                "probabilities": probabilities,
                "confidence": confidence
            })
        }
        Question::Score(criteria) => {
            let level_count = criteria.len();
            let selected_level = if edge {
                None
            } else {
                Some(rng.gen_range(0..level_count))
            };
            let probabilities = indexed_probabilities(level_count, edge, selected_level);
            let legend: Map<String, Value> = criteria
                .iter()
                .enumerate()
                .map(|(index, criterion)| (index.to_string(), criterion.clone()))
                .collect();
            let score = probabilities
                .iter()
                .map(|(index, probability)| {
                    index
                        .parse::<usize>()
                        .expect("generated score key is numeric") as f64
                        * probability
                            .as_f64()
                            .expect("generated probability is numeric")
                })
                .sum::<f64>();
            let confidence = score_confidence(
                &probabilities
                    .values()
                    .map(|probability| {
                        probability
                            .as_f64()
                            .expect("generated probability is numeric")
                    })
                    .collect::<Vec<_>>(),
            );

            json!({
                "type": "score",
                "score": score,
                "legend": legend,
                "probabilities": probabilities,
                "confidence": confidence
            })
        }
        Question::Box => {
            if edge {
                json!({
                    "type": "box",
                    "box": {"top": 0.0, "bottom": 1.0, "left": 0.0, "right": 1.0},
                    "confidence": 0.0
                })
            } else {
                let left = rng.gen_range(0..100_u32);
                let right = rng.gen_range(left + 1..=100_u32);
                let top = rng.gen_range(0..100_u32);
                let bottom = rng.gen_range(top + 1..=100_u32);

                json!({
                    "type": "box",
                    "box": {
                        "top": f64::from(top) / 100.0,
                        "bottom": f64::from(bottom) / 100.0,
                        "left": f64::from(left) / 100.0,
                        "right": f64::from(right) / 100.0
                    },
                    "confidence": 1.0
                })
            }
        }
    }
}

/// Returns TypeSafe's published Choice confidence formula, which scales the
/// highest probability from a uniform distribution to certainty. A single
/// option is certain by default; it has no rivals to confuse it.
///
/// See [TypeSafe confidence](https://docs.typesafe.ai/confidence#how-confidence-is-calculated).
pub fn choice_confidence(p_max: f64, n: usize) -> f64 {
    match n {
        0 => 0.0,
        1 => 1.0,
        _ => {
            let uniform = 1.0 / n as f64;
            (p_max - uniform) / (1.0 - uniform)
        }
    }
}

/// Returns TypeSafe's published Score confidence formula. It compares the
/// probability-weighted distance from the lowest-index mode with the mean
/// absolute deviation of a uniform distribution. Negative confidence is
/// clipped at zero, since sulking is not a useful scale.
///
/// See [TypeSafe confidence](https://docs.typesafe.ai/confidence#how-confidence-is-calculated).
pub fn score_confidence(probs: &[f64]) -> f64 {
    let n = probs.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return 1.0;
    }

    let uniform_probability = 1.0 / n as f64;
    if probs
        .iter()
        .all(|probability| *probability == uniform_probability)
    {
        return 0.0;
    }

    let mut mode = 0;
    for index in 1..n {
        if probs[index] > probs[mode] {
            mode = index;
        }
    }

    let spread = probs
        .iter()
        .enumerate()
        .map(|(index, probability)| probability * index.abs_diff(mode) as f64)
        .sum::<f64>();
    let midpoint = (n - 1) as f64 / 2.0;
    let uniform_mad = (0..n)
        .map(|index| (index as f64 - midpoint).abs())
        .sum::<f64>()
        / n as f64;

    (1.0 - spread / uniform_mad).max(0.0)
}

fn indexed_probabilities(count: usize, edge: bool, selected: Option<usize>) -> Map<String, Value> {
    (0..count)
        .map(|index| {
            let probability = if edge {
                1.0 / count as f64
            } else if selected == Some(index) {
                1.0
            } else {
                0.0
            };
            (index.to_string(), json!(probability))
        })
        .collect()
}

fn choice_probabilities(options: &[&String], edge: bool, selected: usize) -> Map<String, Value> {
    options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let probability = if edge {
                1.0 / options.len() as f64
            } else if selected == index {
                1.0
            } else {
                0.0
            };
            ((*option).clone(), json!(probability))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rand::{rngs::StdRng, SeedableRng};
    use serde_json::{json, Map, Value};

    use super::{answer, choice_confidence, score_confidence};
    use crate::models::Question;

    fn seeded(seed: u64) -> StdRng {
        StdRng::seed_from_u64(seed)
    }

    fn number(value: &Value) -> f64 {
        value.as_f64().expect("expected a JSON number")
    }

    fn probabilities(value: &Value) -> &Map<String, Value> {
        value
            .get("probabilities")
            .and_then(Value::as_object)
            .expect("expected a probability object")
    }

    fn key_set(map: &Map<String, Value>) -> BTreeSet<String> {
        map.keys().cloned().collect()
    }

    fn choice_question() -> Question {
        let mut criteria = Map::new();
        criteria.insert("alpha".to_owned(), json!("first"));
        criteria.insert("beta".to_owned(), json!("second"));
        criteria.insert("gamma".to_owned(), json!("third"));
        Question::Choice(criteria)
    }

    fn score_question() -> Question {
        Question::Score(vec![
            Value::Null,
            json!("middle"),
            json!({"label": [1, true]}),
            json!(["last", null]),
        ])
    }

    #[test]
    fn noul_has_a_binary_normal_answer_or_half_on_the_edge_path() {
        let mut normal_rng = seeded(11);
        let normal = answer(&Question::Noul, false, &mut normal_rng);
        assert_eq!(normal.get("type"), Some(&json!("noul")));
        assert_eq!(normal.as_object().unwrap().len(), 2);
        assert!(matches!(number(normal.get("noul").unwrap()), 0.0 | 1.0));
        assert!(normal.get("confidence").is_none());

        let mut edge_rng = seeded(11);
        let edge = answer(&Question::Noul, true, &mut edge_rng);
        assert_eq!(number(edge.get("noul").unwrap()), 0.5);
        assert!(edge.get("confidence").is_none());
    }

    #[test]
    fn choice_normal_and_edge_answers_keep_every_option_and_sum_to_one() {
        let question = choice_question();
        let expected = ["alpha", "beta", "gamma"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();

        let mut normal_rng = seeded(22);
        let normal = answer(&question, false, &mut normal_rng);
        let normal_probabilities = probabilities(&normal);
        assert_eq!(normal.get("type"), Some(&json!("choice")));
        assert_eq!(key_set(normal_probabilities), expected);
        let winner = normal.get("choice").and_then(Value::as_str).unwrap();
        assert_eq!(number(&normal_probabilities[winner]), 1.0);
        assert!(normal_probabilities
            .iter()
            .filter(|(key, _)| key.as_str() != winner)
            .all(|(_, probability)| number(probability) == 0.0));
        assert_eq!(normal_probabilities.values().map(number).sum::<f64>(), 1.0);
        assert_eq!(number(normal.get("confidence").unwrap()), 1.0);

        let mut edge_rng = seeded(22);
        let edge = answer(&question, true, &mut edge_rng);
        let edge_probabilities = probabilities(&edge);
        assert_eq!(key_set(edge_probabilities), expected);
        assert!(expected.contains(edge.get("choice").and_then(Value::as_str).unwrap()));
        assert!(edge_probabilities
            .values()
            .all(|probability| number(probability) == 1.0 / 3.0));
        assert!((edge_probabilities.values().map(number).sum::<f64>() - 1.0).abs() < 1e-12);
        assert_eq!(number(edge.get("confidence").unwrap()), 0.0);
    }

    #[test]
    fn singleton_choice_is_certain_even_on_the_edge_path() {
        let mut criteria = Map::new();
        criteria.insert("only".to_owned(), Value::Null);
        let question = Question::Choice(criteria);

        for force_edge in [false, true] {
            let mut rng = seeded(33);
            let result = answer(&question, force_edge, &mut rng);
            assert_eq!(result.get("choice"), Some(&json!("only")));
            assert_eq!(number(&probabilities(&result)["only"]), 1.0);
            assert_eq!(number(result.get("confidence").unwrap()), 1.0);
        }
    }

    #[test]
    fn score_preserves_criterion_values_and_scores_the_returned_distribution() {
        let question = score_question();
        let Question::Score(criteria) = &question else {
            unreachable!();
        };
        let expected_legend: Map<String, Value> = criteria
            .iter()
            .enumerate()
            .map(|(index, value)| (index.to_string(), value.clone()))
            .collect();

        let mut normal_rng = seeded(44);
        let normal = answer(&question, false, &mut normal_rng);
        let normal_probabilities = probabilities(&normal);
        assert_eq!(normal.get("type"), Some(&json!("score")));
        assert_eq!(normal.get("legend"), Some(&json!(expected_legend)));
        assert_eq!(
            key_set(normal_probabilities),
            (0..criteria.len()).map(|index| index.to_string()).collect()
        );
        assert_eq!(normal_probabilities.values().map(number).sum::<f64>(), 1.0);
        assert_eq!(
            normal_probabilities
                .values()
                .filter(|p| number(p) == 1.0)
                .count(),
            1
        );
        assert_eq!(number(normal.get("confidence").unwrap()), 1.0);

        let recomputed_normal_score = normal_probabilities
            .iter()
            .map(|(index, probability)| {
                index.parse::<usize>().unwrap() as f64 * number(probability)
            })
            .sum::<f64>();
        assert_eq!(
            number(normal.get("score").unwrap()),
            recomputed_normal_score
        );

        let mut edge_rng = seeded(44);
        let edge = answer(&question, true, &mut edge_rng);
        let edge_probabilities = probabilities(&edge);
        assert_eq!(edge.get("legend"), Some(&json!(expected_legend)));
        assert!(edge_probabilities
            .values()
            .all(|probability| number(probability) == 0.25));
        assert_eq!(edge_probabilities.values().map(number).sum::<f64>(), 1.0);
        let recomputed_edge_score = edge_probabilities
            .iter()
            .map(|(index, probability)| {
                index.parse::<usize>().unwrap() as f64 * number(probability)
            })
            .sum::<f64>();
        assert_eq!(number(edge.get("score").unwrap()), recomputed_edge_score);
        assert_eq!(number(edge.get("score").unwrap()), 1.5);
        assert_eq!(number(edge.get("confidence").unwrap()), 0.0);
    }

    #[test]
    fn box_normal_coordinates_use_nonempty_hundredth_grid_rectangles() {
        let mut rng = seeded(55);
        let result = answer(&Question::Box, false, &mut rng);
        assert_eq!(result.get("type"), Some(&json!("box")));
        assert_eq!(number(result.get("confidence").unwrap()), 1.0);

        let rect = result.get("box").and_then(Value::as_object).unwrap();
        assert_eq!(rect.len(), 4);
        let top = number(&rect["top"]);
        let bottom = number(&rect["bottom"]);
        let left = number(&rect["left"]);
        let right = number(&rect["right"]);
        assert!((0.0..=0.99).contains(&left));
        assert!((0.01..=1.0).contains(&right));
        assert!((0.0..=0.99).contains(&top));
        assert!((0.01..=1.0).contains(&bottom));
        assert!(right > left);
        assert!(bottom > top);
        for coordinate in [top, bottom, left, right] {
            assert!((coordinate * 100.0 - (coordinate * 100.0).round()).abs() < 1e-10);
        }
    }

    #[test]
    fn box_edge_path_is_the_full_image() {
        let mut rng = seeded(66);
        let result = answer(&Question::Box, true, &mut rng);
        let rect = result.get("box").and_then(Value::as_object).unwrap();
        assert_eq!(number(&rect["top"]), 0.0);
        assert_eq!(number(&rect["left"]), 0.0);
        assert_eq!(number(&rect["bottom"]), 1.0);
        assert_eq!(number(&rect["right"]), 1.0);
        assert_eq!(number(result.get("confidence").unwrap()), 0.0);
    }

    #[test]
    fn confidence_matches_type_safe_choice_and_score_examples() {
        assert!((choice_confidence(0.6, 3) - 0.4).abs() < 1e-12);
        assert_eq!(choice_confidence(1.0 / 3.0, 3), 0.0);
        assert_eq!(choice_confidence(1.0, 1), 1.0);

        assert!((score_confidence(&[0.0, 0.57, 0.43]) - 0.355).abs() < 1e-12);
        assert_eq!(score_confidence(&[0.5, 0.0, 0.5]), 0.0);
        assert!((score_confidence(&[0.0, 0.5, 0.5]) - 0.25).abs() < 1e-12);
        assert_eq!(score_confidence(&[1.0]), 1.0);

        // Equal maxima use the lowest index as the mode, as in TypeSafe's formula.
        assert_eq!(score_confidence(&[0.35, 0.0, 0.35, 0.3, 0.0]), 0.0);
    }
}
