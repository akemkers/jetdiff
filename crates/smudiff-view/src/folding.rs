//! Folding av uendrede linjer. Port av `FoldingModelSupport`: `FoldingBuilderBase` bygger
//! nestede nivåer per gap, `FoldedBlock` holder regionene parret på begge sider, og
//! `ExpandSuggester` bevarer tilstanden når foldene bygges på nytt.

use std::ops::Range;

use smudiff_core::side::Side;

use crate::layout::{LineLayout, VisualFold, visual_folds};
use crate::model::{FileDiff, idx};

/// `TextDiffSettingsHolder.CONTEXT_RANGE_MODES`. `None` er -1, aldri fold.
pub const CONTEXT_RANGE_MODES: [Option<usize>; 5] = [Some(1), Some(2), Some(4), Some(8), None];

/// `TextDiffSettings.contextRange`.
pub const DEFAULT_CONTEXT_RANGE: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldSettings {
    pub context_range: Option<usize>,
    /// `EXPAND_BY_DEFAULT`.
    pub expand_by_default: bool,
}

impl Default for FoldSettings {
    fn default() -> Self {
        Self {
            context_range: Some(DEFAULT_CONTEXT_RANGE),
            // Kollapset, som commit-dialogen og VCS-loggen i `TextDiffSettingsHolder`
            expand_by_default: false,
        }
    }
}

impl FoldSettings {
    /// Neste verdi i `CONTEXT_RANGE_MODES`, og rundt til starten.
    pub fn next_context_range(self) -> Self {
        let i = CONTEXT_RANGE_MODES
            .iter()
            .position(|&m| m == self.context_range)
            .map_or(0, |i| i + 1);
        Self {
            context_range: CONTEXT_RANGE_MODES[i % CONTEXT_RANGE_MODES.len()],
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldBlock {
    /// `None` når siden har under to linjer å folde.
    pub lines: [Option<Range<usize>>; 2],
    /// Felles for sidene, siden `MyFoldingListener` speiler tilstanden.
    pub expanded: bool,
}

impl FoldBlock {
    pub fn lines(&self, side: Side) -> Option<Range<usize>> {
        self.lines[idx(side)].clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FoldId {
    pub group: usize,
    /// 0 er ytterst, med minst kontekst.
    pub block: usize,
}

/// Kollapset fold som vises, med id for klikk og hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connector {
    pub id: FoldId,
    /// Første linje i folden per side.
    pub lines: [usize; 2],
    pub hovered: bool,
}

type Regions = [Option<Range<usize>>; 2];

#[derive(Debug, Clone)]
pub struct Folding {
    settings: FoldSettings,
    line_counts: [usize; 2],
    /// `FoldedGroup`: nestede blokker per gap, ytterst først.
    groups: Vec<Vec<FoldBlock>>,
    visible: [Vec<VisualFold>; 2],
    visible_ids: [Vec<FoldId>; 2],
    hovered: Option<FoldId>,
}

impl Folding {
    pub fn new(diff: &FileDiff, settings: FoldSettings) -> Self {
        Self::build(diff, settings, None)
    }

    /// Ingen folds. For tester og filer der folding er slått av.
    pub fn none(diff: &FileDiff) -> Self {
        Self::new(
            diff,
            FoldSettings {
                context_range: None,
                ..FoldSettings::default()
            },
        )
    }

    /// Nye innstillinger. Åpne og lukkede områder beholdes som i `ExpandSuggester`.
    pub fn with_settings(&self, diff: &FileDiff, settings: FoldSettings) -> Self {
        Self::build(diff, settings, Some(self.cache()))
    }

    fn build(diff: &FileDiff, settings: FoldSettings, cache: Option<FoldingCache>) -> Self {
        let line_counts = [diff.line_count(Side::Left), diff.line_count(Side::Right)];
        let groups = match settings.context_range {
            None => Vec::new(),
            Some(range) => {
                let changes = diff.blocks().iter().map(|b| b.lines.clone());
                let mut suggester = ExpandSuggester::new(cache, settings.expand_by_default);
                build_groups(changes, line_counts, range)
                    .into_iter()
                    .map(|group| {
                        group
                            .into_iter()
                            .map(|lines| FoldBlock {
                                expanded: suggester.is_expanded(&lines),
                                lines,
                            })
                            .collect()
                    })
                    .collect()
            }
        };
        let mut folding = Self {
            settings,
            line_counts,
            groups,
            visible: Default::default(),
            visible_ids: Default::default(),
            hovered: None,
        };
        folding.update();
        folding
    }

    pub fn settings(&self) -> FoldSettings {
        self.settings
    }

    pub fn groups(&self) -> &[Vec<FoldBlock>] {
        &self.groups
    }

    pub fn block(&self, id: FoldId) -> &FoldBlock {
        &self.groups[id.group][id.block]
    }

    pub fn layout(&self, side: Side, line_height: f32) -> LineLayout<'_> {
        LineLayout::with_folds(
            self.line_counts[idx(side)],
            line_height,
            &self.visible[idx(side)],
        )
    }

    pub fn layouts(&self, line_height: f32) -> [LineLayout<'_>; 2] {
        [
            self.layout(Side::Left, line_height),
            self.layout(Side::Right, line_height),
        ]
    }

    /// Kollapsede folds som vises på siden, i rekkefølge.
    pub fn visible(&self, side: Side) -> impl Iterator<Item = (FoldId, &VisualFold)> {
        let i = idx(side);
        self.visible_ids[i].iter().copied().zip(&self.visible[i])
    }

    /// `getBlockForLine`: kollapset fold som starter på linja. Andre linjer i en fold er
    /// skjult, og `yPositionToLogicalLine` gir alltid første linje.
    pub fn fold_at_line(&self, side: Side, line: usize) -> Option<FoldId> {
        let i = idx(side);
        let folds = &self.visible[i];
        let k = folds.partition_point(|f| f.lines.start < line);
        folds
            .get(k)
            .filter(|f| f.lines.start == line)
            .map(|_| self.visible_ids[i][k])
    }

    /// `FoldedBlock.setExpanded`: begge sider. Klikk på en fold åpner neste nivå.
    pub fn set_expanded(&mut self, id: FoldId, expanded: bool) -> bool {
        let block = &mut self.groups[id.group][id.block];
        if block.expanded == expanded {
            return false;
        }
        block.expanded = expanded;
        self.update();
        true
    }

    /// `FoldingModelSupport.expandAll`.
    pub fn expand_all(&mut self, expanded: bool) -> bool {
        let mut changed = false;
        for block in self.groups.iter_mut().flatten() {
            changed |= block.expanded != expanded;
            block.expanded = expanded;
        }
        self.update();
        changed
    }

    /// `ExpandRegionAction` på caret: folden som starter på linja, ellers den ytterste
    /// kollapsede som inneholder den.
    pub fn expand_at(&mut self, side: Side, line: usize) -> bool {
        if let Some(id) = self.unique_start(side, line)
            && !self.block(id).expanded
        {
            return self.set_expanded(id, true);
        }
        let found = self
            .containing(side, line)
            .find(|&id| !self.block(id).expanded);
        found.is_some_and(|id| self.set_expanded(id, true))
    }

    /// `FoldingModelImpl.onFoldProcessingEnd`: caret inne i en kollapset fold flyttes til
    /// starten av den.
    pub fn caret_after(&self, side: Side, line: usize) -> usize {
        let folds = &self.visible[idx(side)];
        let k = folds.partition_point(|f| f.lines.start <= line);
        match k.checked_sub(1).map(|k| &folds[k]) {
            Some(f) if line < f.lines.end => f.lines.start,
            _ => line,
        }
    }

    pub fn hovered(&self) -> Option<FoldId> {
        self.hovered
    }

    pub fn set_hovered(&mut self, id: Option<FoldId>) -> bool {
        let changed = self.hovered != id;
        self.hovered = id;
        changed
    }

    /// `MyPaintable.process`: per gruppe den ytterste blokken som er kollapset på begge
    /// sider. Har én side ingen fold, blir det ingen kobling.
    pub fn connectors(&self) -> impl Iterator<Item = Connector> + '_ {
        self.groups.iter().enumerate().filter_map(|(g, group)| {
            let (b, block) = group.iter().enumerate().find(|(_, b)| !b.expanded)?;
            let [Some(l), Some(r)] = &block.lines else {
                // Indre blokker er mindre, så de mangler også en side
                return None;
            };
            let id = FoldId { group: g, block: b };
            Some(Connector {
                id,
                lines: [l.start, r.start],
                hovered: self.hovered == Some(id),
            })
        })
    }

    /// `FoldingUtil.findFoldRegionStartingAtLine`: bare hvis nøyaktig én region starter der.
    fn unique_start(&self, side: Side, line: usize) -> Option<FoldId> {
        let mut found = None;
        for (id, block) in self.ids() {
            if block.lines(side).is_some_and(|r| r.start == line) {
                if found.is_some() {
                    return None;
                }
                found = Some(id);
            }
        }
        found
    }

    /// Blokker med region som inneholder linja, ytterst først.
    fn containing(&self, side: Side, line: usize) -> impl Iterator<Item = FoldId> + '_ {
        self.ids()
            .filter(move |(_, b)| b.lines(side).is_some_and(|r| r.contains(&line)))
            .map(|(id, _)| id)
    }

    fn ids(&self) -> impl Iterator<Item = (FoldId, &FoldBlock)> {
        self.groups.iter().enumerate().flat_map(|(g, group)| {
            group
                .iter()
                .enumerate()
                .map(move |(b, block)| (FoldId { group: g, block: b }, block))
        })
    }

    /// Ytterste kollapsede region per gruppe og side. Det er den `FoldingModel` viser, og
    /// den eneste med separator (`getHighlighterCondition`).
    fn update(&mut self) {
        for side in [Side::Left, Side::Right] {
            let i = idx(side);
            let (ids, ranges): (Vec<_>, Vec<_>) = self
                .groups
                .iter()
                .enumerate()
                .filter_map(|(g, group)| {
                    group.iter().enumerate().find_map(|(b, block)| {
                        let lines = block.lines(side)?;
                        (!block.expanded).then_some((FoldId { group: g, block: b }, lines))
                    })
                })
                .unzip();
            self.visible[i] = visual_folds(ranges);
            self.visible_ids[i] = ids;
        }
        if self
            .hovered
            .is_some_and(|id| !self.visible_ids.iter().flatten().any(|&v| v == id))
        {
            self.hovered = None;
        }
    }

    /// `collectFoldedGroupsStates`: ytterste åpne og ytterste lukkede per gruppe.
    fn cache(&self) -> FoldingCache {
        let ranges = [Side::Left, Side::Right].map(|side| {
            self.groups
                .iter()
                .filter_map(|group| {
                    let mut state = GroupState::default();
                    for block in group {
                        let Some(lines) = block.lines(side) else {
                            continue;
                        };
                        if !block.expanded {
                            state.collapsed = Some(lines);
                            break;
                        }
                        state.expanded.get_or_insert(lines);
                    }
                    (state.expanded.is_some() || state.collapsed.is_some()).then_some(state)
                })
                .collect()
        });
        FoldingCache {
            ranges,
            expand_by_default: self.settings.expand_by_default,
            // `updateContext` skriver ikke over cachen med en tom tilstand
            empty: self.groups.is_empty(),
        }
    }
}

/// `FoldingBuilderBase.build` og `addGroup`: ett gap mellom hver blokk, pluss før første
/// og etter siste. Nivåene har `range`, `2 * range` og `4 * range` linjer kontekst.
pub fn build_groups(
    changes: impl IntoIterator<Item = [Range<usize>; 2]>,
    line_counts: [usize; 2],
    range: usize,
) -> Vec<Vec<Regions>> {
    let mut groups = Vec::new();
    let counts = line_counts.map(|c| c as i64);
    let mut add = |starts: [i64; 2], ends: [i64; 2]| {
        let group: Vec<Regions> = [range, range * 2, range * 4]
            .into_iter()
            .filter_map(|shift| {
                let shift = shift as i64;
                let regions: Regions = std::array::from_fn(|i| {
                    let start = starts[i].saturating_add(shift).clamp(0, counts[i]);
                    let end = ends[i].saturating_sub(shift).clamp(0, counts[i]);
                    (end - start >= 2).then_some(start as usize..end as usize)
                });
                regions.iter().any(Option::is_some).then_some(regions)
            })
            .collect();
        if !group.is_empty() {
            groups.push(group);
        }
    };
    // `Integer.MIN_VALUE` og `MAX_VALUE`: første og siste gap foldes helt ut til kanten
    let mut last = [i64::MIN; 2];
    for c in changes {
        let starts = last;
        last = [c[0].end as i64, c[1].end as i64];
        add(starts, [c[0].start as i64, c[1].start as i64]);
    }
    add(last, [i64::MAX; 2]);
    groups
}

#[derive(Debug, Clone, Default)]
struct GroupState {
    expanded: Option<Range<usize>>,
    collapsed: Option<Range<usize>>,
}

impl GroupState {
    fn range(&self) -> &Range<usize> {
        self.expanded.as_ref().or(self.collapsed.as_ref()).unwrap()
    }
}

#[derive(Debug, Clone)]
struct FoldingCache {
    ranges: [Vec<GroupState>; 2],
    expand_by_default: bool,
    empty: bool,
}

fn contains(outer: &Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// `ExpandSuggester`. Blokkene spørres i rekkefølge, så en markør per side holder.
struct ExpandSuggester {
    cache: Option<FoldingCache>,
    index: [usize; 2],
    default: bool,
}

impl ExpandSuggester {
    fn new(cache: Option<FoldingCache>, default: bool) -> Self {
        Self {
            cache: cache.filter(|c| !c.empty),
            index: [0, 0],
            default,
        }
    }

    fn is_expanded(&mut self, regions: &Regions) -> bool {
        match &self.cache {
            Some(c) if c.expand_by_default == self.default => {}
            _ => return self.default,
        }
        let mut state = None;
        for (i, range) in regions.iter().enumerate() {
            let Some(range) = range else {
                continue;
            };
            let Some(side_state) = self.cached_expanded(range, i) else {
                continue;
            };
            match state {
                None => state = Some(side_state),
                Some(s) if s != side_state => return self.default,
                Some(_) => {}
            }
        }
        state.unwrap_or(self.default)
    }

    fn cached_expanded(&mut self, range: &Range<usize>, i: usize) -> Option<bool> {
        let state = self.cached_state(range, i)?;
        if state.collapsed.as_ref().is_some_and(|c| contains(c, range)) {
            return Some(false);
        }
        if state.expanded.as_ref().is_some_and(|e| contains(e, range)) {
            return Some(true);
        }
        None
    }

    fn cached_state(&mut self, range: &Range<usize>, i: usize) -> Option<&GroupState> {
        if range.is_empty() {
            return None;
        }
        let ranges = &self.cache.as_ref()?.ranges[i];
        while let Some(state) = ranges.get(self.index[i]) {
            let r = state.range();
            if contains(r, range) {
                return Some(state);
            }
            // Kan trengs for neste nivå inni samme gruppe
            if r.end > range.start && r.start >= range.start {
                return None;
            }
            self.index[i] += 1;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smudiff_core::ComparisonPolicy;

    const L: Side = Side::Left;
    const R: Side = Side::Right;

    fn diff(a: &str, b: &str) -> FileDiff {
        FileDiff::new(a, b, ComparisonPolicy::Default).unwrap()
    }

    fn lines(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("{prefix}{i}\n")).collect()
    }

    /// Endringer på linjene 10, 40 og 90 av 100.
    fn three_changes() -> FileDiff {
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [10, 40, 90] {
            b[i] = format!("x{i}");
        }
        diff(&a, &(b.join("\n") + "\n"))
    }

    fn both(r: Range<usize>) -> Regions {
        [Some(r.clone()), Some(r)]
    }

    #[test]
    fn grupper_med_tre_nivaer() {
        // 101 linjer med den tomme etter siste '\n'
        let groups = build_groups(
            [[10..11, 10..11], [40..41, 40..41], [90..91, 90..91]],
            [101, 101],
            4,
        );
        assert_eq!(
            groups,
            vec![
                // Første gap går helt til toppen, så alle nivåene starter på 0
                vec![both(0..6), both(0..2)],
                // 16 linjer kontekst gir 27..24, altså ingen region
                vec![both(15..36), both(19..32)],
                vec![both(45..86), both(49..82), both(57..74)],
                vec![both(95..101), both(99..101)],
            ]
        );
    }

    #[test]
    fn minst_to_linjer_per_side() {
        // Gap på 9 linjer: 4 kontekst på hver side gir én linje, altså ingen fold
        let groups = build_groups([[0..1, 0..1], [10..11, 10..11]], [11, 11], 4);
        assert!(groups.iter().flatten().all(|r| r != &both(5..6)));
        assert_eq!(groups.len(), 0);
        // Ulik lengde: bare den lange siden får fold
        let groups = build_groups([[0..1, 0..1], [10..11, 20..21]], [11, 21], 4);
        assert_eq!(groups[0][0], [None, Some(5..16)]);
    }

    #[test]
    fn like_filer_foldes_helt() {
        let d = diff(&lines("a", 30), &lines("a", 30));
        let f = Folding::new(&d, FoldSettings::default());
        assert_eq!(f.groups().len(), 1);
        assert_eq!(f.groups()[0][0].lines, both(0..31));
        assert_eq!(f.layout(L, 10.).visual_line_count(), 1);
    }

    #[test]
    fn kontekst_av_gir_ingen_folds() {
        let d = three_changes();
        let f = Folding::none(&d);
        assert!(f.groups().is_empty());
        assert_eq!(f.layout(L, 10.).visual_line_count(), 101);
        assert_eq!(f.connectors().count(), 0);
    }

    #[test]
    fn kontekst_sykler_som_intellij() {
        let mut s = FoldSettings::default();
        assert_eq!(s.context_range, Some(4));
        let mut seen = vec![];
        for _ in 0..5 {
            s = s.next_context_range();
            seen.push(s.context_range);
        }
        assert_eq!(seen, [Some(8), None, Some(1), Some(2), Some(4)]);
    }

    #[test]
    fn klikk_apner_ett_niva_om_gangen() {
        let d = three_changes();
        let mut f = Folding::new(&d, FoldSettings::default());
        let visible = |f: &Folding| -> Vec<Range<usize>> {
            f.visible(L).map(|(_, v)| v.lines.clone()).collect()
        };
        assert_eq!(visible(&f), [0..6, 15..36, 45..86, 95..101]);
        let id = f.fold_at_line(L, 45).unwrap();
        assert_eq!(id, FoldId { group: 2, block: 0 });
        assert_eq!(f.fold_at_line(R, 45), Some(id));
        assert_eq!(f.fold_at_line(L, 46), None);

        assert!(f.set_expanded(id, true));
        assert_eq!(visible(&f)[2], 49..82);
        let id = f.fold_at_line(R, 49).unwrap();
        f.set_expanded(id, true);
        assert_eq!(visible(&f)[2], 57..74);
        let id = f.fold_at_line(L, 57).unwrap();
        f.set_expanded(id, true);
        assert_eq!(visible(&f), [0..6, 15..36, 95..101]);
        // Begge sider følger
        assert_eq!(
            visible(&f),
            f.visible(R)
                .map(|(_, v)| v.lines.clone())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn expand_og_collapse_alle() {
        let d = three_changes();
        let mut f = Folding::new(&d, FoldSettings::default());
        assert!(f.expand_all(true));
        assert_eq!(f.visible(L).count(), 0);
        assert_eq!(f.layout(R, 10.).visual_line_count(), 101);
        assert!(!f.expand_all(true));
        f.expand_all(false);
        assert_eq!(f.visible(L).count(), 4);
        // Utvidet som standard
        let f = Folding::new(
            &d,
            FoldSettings {
                expand_by_default: true,
                ..FoldSettings::default()
            },
        );
        assert_eq!(f.visible(L).count(), 0);
    }

    #[test]
    fn expand_pa_caret() {
        let d = three_changes();
        let mut f = Folding::new(&d, FoldSettings::default());
        // Caret på foldens linje: ytterste kollapsede
        assert!(f.expand_at(L, 45));
        assert_eq!(f.fold_at_line(L, 49), Some(FoldId { group: 2, block: 1 }));
        // Linje 60 ligger i alle tre nivåene, ytterste kollapsede er nivå 1
        assert!(f.expand_at(L, 60));
        assert_eq!(f.fold_at_line(L, 57), Some(FoldId { group: 2, block: 2 }));
        // Linje 0: alle nivåene starter der, så den faller tilbake til inneholdende
        assert!(f.expand_at(L, 0));
        assert_eq!(f.fold_at_line(L, 0), Some(FoldId { group: 0, block: 1 }));
        // Inne i en endring er det ingenting
        assert!(!f.expand_at(L, 10));
    }

    #[test]
    fn caret_i_kollapset_fold_flyttes_til_starten() {
        let d = three_changes();
        let f = Folding::new(&d, FoldSettings::default());
        assert_eq!(f.caret_after(L, 50), 45);
        assert_eq!(f.caret_after(L, 45), 45);
        assert_eq!(f.caret_after(L, 40), 40);
        assert_eq!(f.caret_after(L, 3), 0);
    }

    #[test]
    fn koblinger_krever_fold_pa_begge_sider() {
        // Høyre har 20 ekstra linjer etter linje 10: venstre gap 10..20 blir for lite
        // på innerste nivå, men ikke høyre
        let a = lines("a", 30);
        let b = format!(
            "{}{}{}",
            lines("a", 10),
            lines("x", 20),
            &a[lines("a", 10).len()..]
        );
        let d = diff(&a, &b);
        assert_eq!(d.blocks()[0].lines, [10..10, 10..30]);
        let mut f = Folding::new(&d, FoldSettings::default());
        let c: Vec<_> = f.connectors().collect();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].lines, [0, 0]);
        assert_eq!(c[1].lines, [14, 34]);
        f.set_hovered(Some(c[1].id));
        assert!(f.connectors().nth(1).unwrap().hovered);

        // Kun én side har fold: ingen kobling
        let groups = build_groups([[0..1, 0..1], [10..11, 20..21]], [11, 21], 4);
        assert_eq!(groups[0][0].clone(), [None, Some(5..16)]);
        let d = diff(
            &(lines("a", 1) + &lines("b", 9) + "c\n"),
            &(lines("a", 1) + &lines("b", 19) + "c\n"),
        );
        let f = Folding::new(&d, FoldSettings::default());
        assert!(f.connectors().all(|c| c.lines[0] != 5));
    }

    #[test]
    fn hover_nullstilles_nar_folden_forsvinner() {
        let d = three_changes();
        let mut f = Folding::new(&d, FoldSettings::default());
        let id = f.fold_at_line(L, 45).unwrap();
        assert!(f.set_hovered(Some(id)));
        assert!(!f.set_hovered(Some(id)));
        f.set_expanded(id, true);
        assert_eq!(f.hovered(), None);
    }

    #[test]
    fn tilstand_bevares_nar_konteksten_endres() {
        let d = three_changes();
        let mut f = Folding::new(&d, FoldSettings::default());
        // Åpne gruppen rundt 45..86 helt
        f.expand_at(L, 60);
        f.expand_at(L, 60);
        f.expand_at(L, 60);
        assert_eq!(f.fold_at_line(L, 57), None);
        let f8 = f.with_settings(&d, f.settings().next_context_range());
        assert_eq!(f8.settings().context_range, Some(8));
        let visible: Vec<_> = f8.visible(L).map(|(_, v)| v.lines.clone()).collect();
        // Gruppen 49..82 er åpen, de andre kollapset med 8 linjer kontekst
        assert_eq!(visible, [0..2, 19..32, 99..101]);
        // Byttes standarden brukes den
        let open = f.with_settings(
            &d,
            FoldSettings {
                expand_by_default: true,
                ..f.settings()
            },
        );
        assert_eq!(open.visible(L).count(), 0);
    }
}
