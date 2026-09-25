//! Port av `UniqueLCS.kt`: LCS over elementer som finnes nøyaktig én gang på hver side.

use std::collections::HashMap;

/// Returnerer matchede posisjoner (relativt til start) på hver side, eller `None` uten unike par.
pub(crate) fn unique_lcs(
    first: &[u32],
    second: &[u32],
    start1: usize,
    count1: usize,
    start2: usize,
    count2: usize,
) -> Option<(Vec<usize>, Vec<usize>)> {
    // map: key -> (offset1 + 1), -1 hvis ikke unik
    // matching: offset1 -> (offset2 + 1)
    let mut map: HashMap<u32, i64> = HashMap::with_capacity(count1 + count2);
    let mut matching = vec![0usize; count1];

    for i in 0..count1 {
        let key = first[start1 + i];
        match map.get(&key).copied().unwrap_or(0) {
            -1 => {}
            0 => {
                map.insert(key, i as i64 + 1);
            }
            _ => {
                map.insert(key, -1);
            }
        }
    }

    let mut count: i64 = 0;
    for i in 0..count2 {
        let key = second[start2 + i];
        let value = map.get(&key).copied().unwrap_or(0);
        if value == 0 || value == -1 {
            continue;
        }
        let slot = (value - 1) as usize;
        if matching[slot] == 0 {
            matching[slot] = i + 1;
            count += 1;
        } else {
            matching[slot] = 0;
            map.insert(key, -1);
            count -= 1;
        }
    }

    if count == 0 {
        return None;
    }

    // Lengste stigende delsekvens over de unike parene.
    let count = count as usize;
    let mut sequence = vec![0usize; count];
    let mut last_element = vec![0usize; count];
    let mut predecessor = vec![-1isize; count1];

    let mut length = 0;
    for i in 0..count1 {
        if matching[i] == 0 {
            continue;
        }
        let j = sequence[..length].partition_point(|&v| v < matching[i]);
        debug_assert!(j == length || sequence[j] != matching[i]);
        if j == length || matching[i] < sequence[j] {
            sequence[j] = matching[i];
            last_element[j] = i;
            predecessor[i] = if j > 0 {
                last_element[j - 1] as isize
            } else {
                -1
            };
            if j == length {
                length += 1;
            }
        }
    }

    let mut r1 = vec![0usize; length];
    let mut r2 = vec![0usize; length];
    let mut curr = last_element[length - 1] as isize;
    let mut i = length;
    while curr != -1 {
        i -= 1;
        r1[i] = curr as usize;
        r2[i] = matching[curr as usize] - 1;
        curr = predecessor[curr as usize];
    }
    Some((r1, r2))
}
