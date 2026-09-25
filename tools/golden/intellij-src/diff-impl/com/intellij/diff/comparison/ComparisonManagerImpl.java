// Copyright 2000-2024 JetBrains s.r.o. and contributors. Use of this source code is governed by the Apache 2.0 license.
//
// Trimmed copy of platform/diff-impl/src/com/intellij/diff/comparison/ComparisonManagerImpl.java
// (intellij-community@7af191d46d84acf6e30e077df05af9a4c9fe9005) for the jetdiff golden generator.
// Only the two-side line/word paths and processBlocks are kept. Changes vs. the original:
//  - ProgressIndicator -> CancellationChecker.EMPTY (ByLine/ByWord wrappers call *Rt directly)
//  - Registry.is("diff.by.word.deprioritize.line.differences") -> constant false (IDE default,
//    intellij.platform.diff.impl.xml)
//  - ContainerUtil/Consumer replaced by plain Java equivalents
//  - no ComparisonManager superclass
// The algorithm logic is unchanged.
package com.intellij.diff.comparison;

import com.intellij.diff.comparison.ByWordRt.LineBlock;
import com.intellij.diff.comparison.iterables.DiffIterable;
import com.intellij.diff.comparison.iterables.FairDiffIterable;
import com.intellij.diff.fragments.DiffFragment;
import com.intellij.diff.fragments.DiffFragmentImpl;
import com.intellij.diff.fragments.LineFragment;
import com.intellij.diff.fragments.LineFragmentImpl;
import com.intellij.diff.tools.util.text.LineOffsets;
import com.intellij.diff.tools.util.text.LineOffsetsImpl;
import com.intellij.diff.util.Range;
import com.intellij.util.IntPair;
import com.intellij.util.diff.DiffConfig;
import com.intellij.util.text.CharSequenceSubSequence;
import org.jetbrains.annotations.NotNull;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.function.Consumer;

import static java.util.Collections.singletonList;

public final class ComparisonManagerImpl {
  // Registry "diff.by.word.deprioritize.line.differences", defaultValue="false"
  private static final boolean DEPRIORITIZE_LINE_DIFFERENCES = false;

  private static final CancellationChecker INDICATOR = CancellationChecker.EMPTY;

  public static @NotNull List<LineFragment> compareLines(@NotNull CharSequence text1,
                                                         @NotNull CharSequence text2,
                                                         @NotNull LineOffsets lineOffsets1,
                                                         @NotNull LineOffsets lineOffsets2,
                                                         @NotNull ComparisonPolicy policy) throws DiffTooBigException {
    Range boundaryRange = new Range(0, lineOffsets1.getLineCount(),
                                    0, lineOffsets2.getLineCount());
    return compareLines(boundaryRange, text1, text2, lineOffsets1, lineOffsets2, policy);
  }

  public static @NotNull List<LineFragment> compareLines(@NotNull Range boundaryRange,
                                                         @NotNull CharSequence text1,
                                                         @NotNull CharSequence text2,
                                                         @NotNull LineOffsets lineOffsets1,
                                                         @NotNull LineOffsets lineOffsets2,
                                                         @NotNull ComparisonPolicy policy) throws DiffTooBigException {
    List<CharSequence> lineTexts1 = getLineContents(boundaryRange.start1, boundaryRange.end1, text1, lineOffsets1);
    List<CharSequence> lineTexts2 = getLineContents(boundaryRange.start2, boundaryRange.end2, text2, lineOffsets2);

    FairDiffIterable iterable = ByLineRt.compare(lineTexts1, lineTexts2, policy, INDICATOR);
    return convertIntoLineFragments(boundaryRange, lineOffsets1, lineOffsets2, iterable);
  }

  public static @NotNull List<LineFragment> compareLinesInner(@NotNull CharSequence text1,
                                                              @NotNull CharSequence text2,
                                                              @NotNull LineOffsets lineOffsets1,
                                                              @NotNull LineOffsets lineOffsets2,
                                                              @NotNull ComparisonPolicy policy,
                                                              @NotNull InnerFragmentsPolicy fragmentsPolicy) throws DiffTooBigException {
    if (fragmentsPolicy == InnerFragmentsPolicy.WORDS &&
        DEPRIORITIZE_LINE_DIFFERENCES) {
      throw new UnsupportedOperationException("compareLinesWordFirst is not part of the golden generator");
    }

    List<LineFragment> lineFragments = compareLines(text1, text2, lineOffsets1, lineOffsets2, policy);
    if (fragmentsPolicy != InnerFragmentsPolicy.NONE) {
      return createInnerFragments(lineFragments, text1, text2, policy, fragmentsPolicy);
    }
    else {
      return lineFragments;
    }
  }

  private static List<LineFragment> createInnerFragments(@NotNull List<? extends LineFragment> lineFragments,
                                                         @NotNull CharSequence text1,
                                                         @NotNull CharSequence text2,
                                                         @NotNull ComparisonPolicy policy,
                                                         @NotNull InnerFragmentsPolicy fragmentsPolicy) {
    List<LineFragment> result = new ArrayList<>(lineFragments.size());

    int tooBigChunksCount = 0;
    for (LineFragment fragment : lineFragments) {
      assert fragment.getInnerFragments() == null;

      try {
        // Do not try to build fine blocks after few fails
        boolean tryComputeDifferences = tooBigChunksCount < DiffConfig.MAX_BAD_LINES;
        result.addAll(createInnerFragments(fragment, text1, text2, policy, fragmentsPolicy, tryComputeDifferences));
      }
      catch (DiffTooBigException e) {
        result.add(fragment);
        tooBigChunksCount++;
      }
    }

    return result;
  }

  private static @NotNull List<LineFragment> createInnerFragments(@NotNull LineFragment fragment,
                                                                  @NotNull CharSequence text1,
                                                                  @NotNull CharSequence text2,
                                                                  @NotNull ComparisonPolicy policy,
                                                                  @NotNull InnerFragmentsPolicy fragmentsPolicy,
                                                                  boolean tryComputeDifferences) throws DiffTooBigException {
    if (fragmentsPolicy == InnerFragmentsPolicy.NONE) {
      return singletonList(fragment);
    }

    CharSequence subSequence1 = text1.subSequence(fragment.getStartOffset1(), fragment.getEndOffset1());
    CharSequence subSequence2 = text2.subSequence(fragment.getStartOffset2(), fragment.getEndOffset2());

    if (fragment.getStartLine1() == fragment.getEndLine1() ||
        fragment.getStartLine2() == fragment.getEndLine2()) { // Insertion / Deletion
      if (ComparisonUtil.isEqualTexts(subSequence1, subSequence2, policy)) {
        return singletonList(new LineFragmentImpl(fragment, Collections.emptyList()));
      }
      else {
        return singletonList(fragment);
      }
    }

    if (!tryComputeDifferences) return singletonList(fragment);

    if (fragmentsPolicy == InnerFragmentsPolicy.WORDS) {
      return createInnerWordFragments(fragment, subSequence1, subSequence2, policy);
    }
    else if (fragmentsPolicy == InnerFragmentsPolicy.CHARS) {
      return createInnerCharFragments(fragment, subSequence1, subSequence2, policy);
    }
    else {
      throw new IllegalArgumentException(fragmentsPolicy.name());
    }
  }

  private static @NotNull List<LineFragment> createInnerWordFragments(@NotNull LineFragment fragment,
                                                                      @NotNull CharSequence subSequence1,
                                                                      @NotNull CharSequence subSequence2,
                                                                      @NotNull ComparisonPolicy policy) throws DiffTooBigException {
    List<LineBlock> lineBlocks = ByWordRt.compareAndSplit(subSequence1, subSequence2, policy, INDICATOR);
    assert !lineBlocks.isEmpty();

    int startOffset1 = fragment.getStartOffset1();
    int startOffset2 = fragment.getStartOffset2();

    int currentStartLine1 = fragment.getStartLine1();
    int currentStartLine2 = fragment.getStartLine2();

    List<LineFragment> chunks = new ArrayList<>();
    for (int i = 0; i < lineBlocks.size(); i++) {
      LineBlock block = lineBlocks.get(i);
      Range offsets = block.offsets;

      // special case for last line to void problem with empty last line
      int currentEndLine1 = i != lineBlocks.size() - 1 ? currentStartLine1 + block.newlines1 : fragment.getEndLine1();
      int currentEndLine2 = i != lineBlocks.size() - 1 ? currentStartLine2 + block.newlines2 : fragment.getEndLine2();

      chunks.add(new LineFragmentImpl(currentStartLine1, currentEndLine1, currentStartLine2, currentEndLine2,
                                      offsets.start1 + startOffset1, offsets.end1 + startOffset1,
                                      offsets.start2 + startOffset2, offsets.end2 + startOffset2,
                                      block.fragments));

      currentStartLine1 = currentEndLine1;
      currentStartLine2 = currentEndLine2;
    }
    return chunks;
  }

  private static @NotNull List<LineFragment> createInnerCharFragments(@NotNull LineFragment fragment,
                                                                      @NotNull CharSequence subSequence1,
                                                                      @NotNull CharSequence subSequence2,
                                                                      @NotNull ComparisonPolicy policy) throws DiffTooBigException {
    List<DiffFragment> innerChanges = doCompareChars(subSequence1, subSequence2, policy);
    return singletonList(new LineFragmentImpl(fragment, innerChanges));
  }

  private static @NotNull List<DiffFragment> doCompareChars(@NotNull CharSequence text1,
                                                            @NotNull CharSequence text2,
                                                            @NotNull ComparisonPolicy policy) {
    DiffIterable iterable;
    if (policy == ComparisonPolicy.DEFAULT) {
      iterable = ByCharRt.compareTwoStep(text1, text2, INDICATOR);
    }
    else if (policy == ComparisonPolicy.TRIM_WHITESPACES) {
      iterable = ByCharRt.compareTrimWhitespaces(text1, text2, INDICATOR);
    }
    else {
      iterable = ByCharRt.compareIgnoreWhitespaces(text1, text2, INDICATOR);
    }

    return ByWordRt.convertIntoDiffFragments(iterable);
  }

  //
  // Fragments
  //

  public static @NotNull List<LineFragment> convertIntoLineFragments(@NotNull Range boundaryRange,
                                                                     @NotNull LineOffsets lineOffsets1,
                                                                     @NotNull LineOffsets lineOffsets2,
                                                                     @NotNull DiffIterable changes) {
    List<LineFragment> fragments = new ArrayList<>();
    for (Range range : changes.iterateChanges()) {
      int startLine1 = range.start1 + boundaryRange.start1;
      int startLine2 = range.start2 + boundaryRange.start2;
      int endLine1 = range.end1 + boundaryRange.start1;
      int endLine2 = range.end2 + boundaryRange.start2;
      fragments.add(createLineFragment(startLine1, endLine1, startLine2, endLine2, lineOffsets1, lineOffsets2));
    }
    return fragments;
  }

  public static @NotNull LineFragment createLineFragment(int startLine1, int endLine1,
                                                         int startLine2, int endLine2,
                                                         @NotNull LineOffsets lineOffsets1,
                                                         @NotNull LineOffsets lineOffsets2) {
    IntPair offsets1 = getOffsets(lineOffsets1, startLine1, endLine1);
    IntPair offsets2 = getOffsets(lineOffsets2, startLine2, endLine2);
    return new LineFragmentImpl(startLine1, endLine1, startLine2, endLine2,
                                offsets1.first, offsets1.second, offsets2.first, offsets2.second);
  }

  private static @NotNull IntPair getOffsets(@NotNull LineOffsets lineOffsets, int startIndex, int endIndex) {
    if (startIndex == endIndex) {
      int offset;
      if (startIndex < lineOffsets.getLineCount()) {
        offset = lineOffsets.getLineStart(startIndex);
      }
      else {
        offset = lineOffsets.getLineEnd(lineOffsets.getLineCount() - 1, true);
      }
      return new IntPair(offset, offset);
    }
    else {
      int offset1 = lineOffsets.getLineStart(startIndex);
      int offset2 = lineOffsets.getLineEnd(endIndex - 1, true);
      return new IntPair(offset1, offset2);
    }
  }

  //
  // Post process line fragments
  //

  public static @NotNull List<LineFragment> processBlocks(@NotNull List<LineFragment> oldFragments,
                                                          final @NotNull CharSequence text1, final @NotNull CharSequence text2,
                                                          final @NotNull ComparisonPolicy policy,
                                                          final boolean squash, final boolean trim) {
    if (!squash && !trim) return oldFragments;
    if (oldFragments.isEmpty()) return oldFragments;

    final List<LineFragment> newFragments = new ArrayList<>();
    processAdjoining(oldFragments, fragments -> newFragments.addAll(processAdjoining(fragments, text1, text2, policy, squash, trim)));
    return newFragments;
  }

  private static void processAdjoining(@NotNull List<? extends LineFragment> oldFragments,
                                       @NotNull Consumer<? super List<? extends LineFragment>> consumer) {
    int startIndex = 0;
    for (int i = 1; i < oldFragments.size(); i++) {
      if (!isAdjoining(oldFragments.get(i - 1), oldFragments.get(i))) {
        consumer.accept(oldFragments.subList(startIndex, i));
        startIndex = i;
      }
    }
    if (startIndex < oldFragments.size()) {
      consumer.accept(oldFragments.subList(startIndex, oldFragments.size()));
    }
  }

  private static @NotNull List<? extends LineFragment> processAdjoining(@NotNull List<? extends LineFragment> fragments,
                                                                        @NotNull CharSequence text1, @NotNull CharSequence text2,
                                                                        @NotNull ComparisonPolicy policy, boolean squash, boolean trim) {
    int start = 0;
    int end = fragments.size();

    // TODO: trim empty leading/trailing lines
    if (trim && policy == ComparisonPolicy.IGNORE_WHITESPACES) {
      while (start < end) {
        LineFragment fragment = fragments.get(start);
        CharSequenceSubSequence sequence1 = new CharSequenceSubSequence(text1, fragment.getStartOffset1(), fragment.getEndOffset1());
        CharSequenceSubSequence sequence2 = new CharSequenceSubSequence(text2, fragment.getStartOffset2(), fragment.getEndOffset2());

        if ((fragment.getInnerFragments() == null || !fragment.getInnerFragments().isEmpty()) &&
            !ComparisonUtil.isEquals(sequence1, sequence2, ComparisonPolicy.IGNORE_WHITESPACES)) {
          break;
        }
        start++;
      }
      while (start < end) {
        LineFragment fragment = fragments.get(end - 1);
        CharSequenceSubSequence sequence1 = new CharSequenceSubSequence(text1, fragment.getStartOffset1(), fragment.getEndOffset1());
        CharSequenceSubSequence sequence2 = new CharSequenceSubSequence(text2, fragment.getStartOffset2(), fragment.getEndOffset2());

        if ((fragment.getInnerFragments() == null || !fragment.getInnerFragments().isEmpty()) &&
            !ComparisonUtil.isEquals(sequence1, sequence2, ComparisonPolicy.IGNORE_WHITESPACES)) {
          break;
        }
        end--;
      }
    }

    if (start == end) return Collections.emptyList();
    if (squash) {
      return singletonList(doSquash(fragments.subList(start, end)));
    }
    return fragments.subList(start, end);
  }

  private static @NotNull LineFragment doSquash(@NotNull List<? extends LineFragment> oldFragments) {
    assert !oldFragments.isEmpty();
    if (oldFragments.size() == 1) return oldFragments.get(0);

    LineFragment firstFragment = oldFragments.get(0);
    LineFragment lastFragment = oldFragments.get(oldFragments.size() - 1);

    List<DiffFragment> newInnerFragments = new ArrayList<>();
    for (LineFragment fragment : oldFragments) {
      for (DiffFragment innerFragment : extractInnerFragments(fragment)) {
        int shift1 = fragment.getStartOffset1() - firstFragment.getStartOffset1();
        int shift2 = fragment.getStartOffset2() - firstFragment.getStartOffset2();

        DiffFragment previousFragment = newInnerFragments.isEmpty() ? null : newInnerFragments.get(newInnerFragments.size() - 1);
        if (previousFragment == null || !isAdjoiningInner(previousFragment, innerFragment, shift1, shift2)) {
          newInnerFragments.add(new DiffFragmentImpl(innerFragment.getStartOffset1() + shift1, innerFragment.getEndOffset1() + shift1,
                                                     innerFragment.getStartOffset2() + shift2, innerFragment.getEndOffset2() + shift2));
        }
        else {
          newInnerFragments.remove(newInnerFragments.size() - 1);
          newInnerFragments.add(new DiffFragmentImpl(previousFragment.getStartOffset1(), innerFragment.getEndOffset1() + shift1,
                                                     previousFragment.getStartOffset2(), innerFragment.getEndOffset2() + shift2));
        }
      }
    }

    return new LineFragmentImpl(firstFragment.getStartLine1(), lastFragment.getEndLine1(),
                                firstFragment.getStartLine2(), lastFragment.getEndLine2(),
                                firstFragment.getStartOffset1(), lastFragment.getEndOffset1(),
                                firstFragment.getStartOffset2(), lastFragment.getEndOffset2(),
                                newInnerFragments);
  }

  private static boolean isAdjoining(@NotNull LineFragment beforeFragment, @NotNull LineFragment afterFragment) {
    if (beforeFragment.getEndLine1() != afterFragment.getStartLine1() ||
        beforeFragment.getEndLine2() != afterFragment.getStartLine2() ||
        beforeFragment.getEndOffset1() != afterFragment.getStartOffset1() ||
        beforeFragment.getEndOffset2() != afterFragment.getStartOffset2()) {
      return false;
    }

    return true;
  }

  private static boolean isAdjoiningInner(@NotNull DiffFragment beforeFragment, @NotNull DiffFragment afterFragment,
                                          int shift1, int shift2) {
    if (beforeFragment.getEndOffset1() != afterFragment.getStartOffset1() + shift1 ||
        beforeFragment.getEndOffset2() != afterFragment.getStartOffset2() + shift2) {
      return false;
    }

    return true;
  }

  private static @NotNull List<DiffFragment> extractInnerFragments(@NotNull LineFragment lineFragment) {
    if (lineFragment.getInnerFragments() != null) return lineFragment.getInnerFragments();

    int length1 = lineFragment.getEndOffset1() - lineFragment.getStartOffset1();
    int length2 = lineFragment.getEndOffset2() - lineFragment.getStartOffset2();
    return singletonList(new DiffFragmentImpl(0, length1, 0, length2));
  }

  private static @NotNull List<CharSequence> getLineContents(int start, int end, @NotNull CharSequence text, @NotNull LineOffsets lineOffsets) {
    List<CharSequence> lines = new ArrayList<>(end - start);
    for (int line = start; line < end; line++) {
      lines.add(new CharSequenceSubSequence(text, lineOffsets.getLineStart(line), lineOffsets.getLineEnd(line)));
    }
    return lines;
  }

  // LineOffsetsUtil.create(CharSequence) delegates here (diff-impl/.../tools/util/text/LineOffsetsUtil.java)
  public static @NotNull LineOffsets createLineOffsets(@NotNull CharSequence text) {
    return LineOffsetsImpl.create(text);
  }
}
