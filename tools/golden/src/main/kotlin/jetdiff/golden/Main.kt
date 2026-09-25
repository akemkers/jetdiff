package jetdiff.golden

import com.intellij.diff.comparison.ComparisonManagerImpl
import com.intellij.diff.comparison.ComparisonPolicy
import com.intellij.diff.comparison.InnerFragmentsPolicy
import com.intellij.diff.fragments.LineFragment
import java.io.File
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import kotlin.system.exitProcess

const val SOURCE = "intellij-community@7af191d4"

val POLICIES = listOf(
  ComparisonPolicy.DEFAULT,
  ComparisonPolicy.TRIM_WHITESPACES,
  ComparisonPolicy.IGNORE_WHITESPACES,
)

fun main(args: Array<String>) {
  if (args.isEmpty()) {
    System.err.println("usage: golden <case-dir>...")
    exitProcess(2)
  }
  var failed = false
  for (arg in args) {
    val dir = File(arg)
    try {
      val json = generate(readText(File(dir, "a.txt")), readText(File(dir, "b.txt")))
      File(dir, "expected.json").writeText(json, Charsets.UTF_8)
      println("ok   ${dir.name}")
    }
    catch (e: Exception) {
      failed = true
      System.err.println("FAIL ${dir.name}: $e")
    }
  }
  if (failed) exitProcess(1)
}

private fun readText(file: File): String {
  val decoder = StandardCharsets.UTF_8.newDecoder()
    .onMalformedInput(CodingErrorAction.REPORT)
    .onUnmappableCharacter(CodingErrorAction.REPORT)
  val text = decoder.decode(ByteBuffer.wrap(file.readBytes())).toString()
  // IntelliJ Document har alltid '\n', CRLF normaliseres foer diff. Vi krever at fixtures er normalisert.
  require('\r' !in text) { "${file.path} contains '\\r', IntelliJ normalizes line separators before diffing" }
  return text
}

fun generate(text1: String, text2: String): String {
  val u1 = Utf8Offsets(text1)
  val u2 = Utf8Offsets(text2)
  val lo1 = ComparisonManagerImpl.createLineOffsets(text1)
  val lo2 = ComparisonManagerImpl.createLineOffsets(text2)

  val out = StringBuilder()
  out.append("{\n")
  out.append("  \"source\": \"").append(SOURCE).append("\",\n")
  out.append("  \"policies\": {\n")
  POLICIES.forEachIndexed { pi, policy ->
    val lines = ComparisonManagerImpl.compareLines(text1, text2, lo1, lo2, policy)
    val words = ComparisonManagerImpl.compareLinesInner(text1, text2, lo1, lo2, policy, InnerFragmentsPolicy.WORDS)
    // Side-by-side viewer, BY_WORD: squash = true, trim = false (se README)
    val viewer = ComparisonManagerImpl.processBlocks(words, text1, text2, policy, true, false)

    out.append("    \"").append(policy.name).append("\": {\n")
    writeList(out, "lines", lines, u1, u2, last = false)
    writeList(out, "words", words, u1, u2, last = false)
    writeList(out, "viewer", viewer, u1, u2, last = true)
    out.append("    }").append(if (pi < POLICIES.size - 1) ",\n" else "\n")
  }
  out.append("  }\n")
  out.append("}\n")
  return out.toString()
}

private fun writeList(out: StringBuilder, name: String, fragments: List<LineFragment>,
                      u1: Utf8Offsets, u2: Utf8Offsets, last: Boolean) {
  out.append("      \"").append(name).append("\": [")
  if (fragments.isEmpty()) {
    out.append("]")
  }
  else {
    out.append("\n")
    fragments.forEachIndexed { i, f ->
      out.append("        ").append(fragmentJson(f, u1, u2))
      out.append(if (i < fragments.size - 1) ",\n" else "\n")
    }
    out.append("      ]")
  }
  out.append(if (last) "\n" else ",\n")
}

private fun fragmentJson(f: LineFragment, u1: Utf8Offsets, u2: Utf8Offsets): String {
  val s1 = u1.at(f.startOffset1)
  val s2 = u2.at(f.startOffset2)
  val sb = StringBuilder()
  sb.append("{ \"l1\": [").append(f.startLine1).append(", ").append(f.endLine1).append("], ")
  sb.append("\"l2\": [").append(f.startLine2).append(", ").append(f.endLine2).append("], ")
  sb.append("\"o1\": [").append(s1).append(", ").append(u1.at(f.endOffset1)).append("], ")
  sb.append("\"o2\": [").append(s2).append(", ").append(u2.at(f.endOffset2)).append("], ")
  sb.append("\"inner\": ")
  val inner = f.innerFragments
  if (inner == null) {
    sb.append("null")
  }
  else {
    sb.append("[")
    inner.forEachIndexed { i, d ->
      if (i > 0) sb.append(", ")
      // Inner-offsets er relative til fragmentstart, konverter via absolutt posisjon
      sb.append("{ \"o1\": [")
        .append(u1.at(f.startOffset1 + d.startOffset1) - s1).append(", ")
        .append(u1.at(f.startOffset1 + d.endOffset1) - s1).append("], ")
      sb.append("\"o2\": [")
        .append(u2.at(f.startOffset2 + d.startOffset2) - s2).append(", ")
        .append(u2.at(f.startOffset2 + d.endOffset2) - s2).append("] }")
    }
    sb.append("]")
  }
  sb.append(" }")
  return sb.toString()
}

/** UTF-16-indeks til UTF-8-byteoffset. */
class Utf8Offsets(private val text: String) {
  private val map = IntArray(text.length + 1)

  init {
    var bytes = 0
    var i = 0
    while (i < text.length) {
      val cp = text.codePointAt(i)
      val units = Character.charCount(cp)
      map[i] = bytes
      if (units == 2) map[i + 1] = -1 // midt i et surrogatpar
      bytes += when {
        cp < 0x80 -> 1
        cp < 0x800 -> 2
        cp < 0x10000 -> 3
        else -> 4
      }
      i += units
    }
    map[text.length] = bytes
  }

  fun at(utf16: Int): Int {
    val b = map[utf16]
    check(b >= 0) { "offset $utf16 splits a surrogate pair" }
    return b
  }
}
