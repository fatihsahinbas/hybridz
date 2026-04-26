# -*- coding: utf-8 -*-
"""
sao dosyası analizi — Column-oriented (de-interleave) transform için gerekçe.
Farklı kayıt boyutları (RECORD_SIZE) deneyerek en iyi periyodu bulur.
"""
import math
from collections import Counter
from pathlib import Path

def entropy(seq):
    if not seq:
        return 0.0
    counts = Counter(seq)
    total = len(seq)
    return sum(-c/total * math.log2(c/total) for c in counts.values())

# sao'yu bul
candidates = [
    Path("corpus/silesia/sao"),
    Path("../corpus/silesia/sao"),
    Path("./sao"),
]
sao_path = None
for p in candidates:
    if p.exists():
        sao_path = p
        break

if sao_path is None:
    print("sao dosyası bulunamadı. corpus/silesia/sao konumunu kontrol edin.")
    exit(1)

print(f"sao dosyası: {sao_path}")
data = sao_path.read_bytes()
n = len(data)
print(f"Boyut: {n:,} byte ({n/1024/1024:.2f} MB)")

raw_e = entropy(data)
print(f"Ham entropi: {raw_e:.3f} bit/byte → teorik limit %{raw_e/8*100:.1f}")
print(f"HybridZ şu anda: ~%94.4 (sıkıştırmıyor)\n")

# Farklı kayıt boyutları dene — sao için bilinen değer 28, ama emin olalım
print("Kayıt boyutu deneyerek en iyi periyodu bulalım:")
print(f"{'RECORD':>8} {'n_div':>10} {'avg_col_entropy':>16} {'teorik_oran':>12}")

best_record = None
best_entropy = raw_e

for record in [8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64]:
    if n % record != 0 and n % record > record * 0.01 * n:
        # Kısmi bölünme olsa bile devam et (sonu kırpar)
        pass
    n_usable = n - (n % record)
    if n_usable < record * 100:
        continue

    # De-interleave
    columns = [bytearray() for _ in range(record)]
    for i in range(n_usable):
        columns[i % record].append(data[i])

    col_entropies = [entropy(col) for col in columns]
    avg_e = sum(col_entropies) / len(col_entropies)
    mark = ""
    if avg_e < best_entropy:
        best_entropy = avg_e
        best_record = record
        mark = "  ◄ en iyi şu ana kadar"
    print(f"{record:>8d} {n_usable:>10d} {avg_e:>16.3f} {avg_e/8*100:>11.1f}%{mark}")

if best_record:
    print(f"\n=== En iyi kayıt boyutu: {best_record} ===")
    # Detaylı analiz
    columns = [bytearray() for _ in range(best_record)]
    n_usable = n - (n % best_record)
    for i in range(n_usable):
        columns[i % best_record].append(data[i])

    print(f"\nPer-column entropy (RECORD={best_record}):")
    for i, col in enumerate(columns):
        e = entropy(col)
        bar = "#" * int(e * 4)
        print(f"  col {i:3d}: {e:.3f} bit/byte  {bar}")

    # Her sütuna ayrı ayrı delta + entropi
    col_delta_e = []
    for col in columns:
        if len(col) < 2:
            col_delta_e.append(0)
            continue
        delta = bytearray([col[0]])
        for k in range(1, len(col)):
            delta.append((col[k] - col[k-1]) & 0xFF)
        col_delta_e.append(entropy(bytes(delta)))
    avg_delta = sum(col_delta_e) / len(col_delta_e)

    print(f"\n=== ÖZET (RECORD={best_record}) ===")
    print(f"Ham byte-byte entropi     : {raw_e:.3f} bit/byte → %{raw_e/8*100:.1f}")
    print(f"De-interleave sonrası     : {best_entropy:.3f} bit/byte → %{best_entropy/8*100:.1f}")
    print(f"De-interleave + delta     : {avg_delta:.3f} bit/byte → %{avg_delta/8*100:.1f}")
    print(f"HybridZ şu an             : ~%94.4")
    print(f"\nBeklenen iyileşme: %{(1 - best_entropy/raw_e)*100:.1f} oranında daha iyi sıkışma")
