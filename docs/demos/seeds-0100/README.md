# Seeds 0.10.0 release pin

The [recording](output.txt) and [timing file](timing.txt) show the effective
Seeds member configuration changing from 0.9.0 on base
`14bb581ab86179af12a903a7be8198d6f493a716` to 0.10.0 on this candidate.
The four target names stay the same; each archive digest changes.
This is a source configuration demonstration, not an installed-host demonstration.
The PR caption identifies the exact demonstrated candidate head.

Reproduce from the candidate checkout:

```sh
git show 14bb581ab86179af12a903a7be8198d6f493a716:members/seeds.toml > /tmp/seeds-before.toml
python3 docs/demos/seeds-0100/run.py /tmp/seeds-before.toml members/seeds.toml
```

Download both recording files and replay with `scriptreplay --log-out output.txt
--log-timing timing.txt`. Playback requires a terminal replay tool.
The small sanitized recording is retained here alongside the existing pin demos.

Separately, the source-built Tambo bump command downloaded and hashed all four
published archives against SHA256SUMS.txt. The Linux native asset passed the
manifest identity and version probes inside a sandbox with no real HOME or
network access. This pins a release; installation uses the normal updater.
