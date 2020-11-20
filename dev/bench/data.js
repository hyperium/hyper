window.BENCHMARK_DATA = {
  "lastUpdate": 1605832112222,
  "repoUrl": "https://github.com/hyperium/hyper",
  "entries": {
    "connect": [
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "0fd7d3c3635fa5a6d2d9e9e9471ae43ca3f09cad",
          "message": "fixup benchmark output.txt",
          "timestamp": "2020-11-16T14:31:59-08:00",
          "tree_id": "4a3e2b0368c81605beafc6f4da7026519a929f19",
          "url": "https://github.com/hyperium/hyper/commit/0fd7d3c3635fa5a6d2d9e9e9471ae43ca3f09cad"
        },
        "date": 1605566086676,
        "tool": "cargo",
        "benches": [
          {
            "name": "http_connector",
            "value": 39056,
            "range": "± 8021",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "af96ddf008540ca4799381efb16a74af4dc3db28",
          "message": "Use patched GH action",
          "timestamp": "2020-11-16T16:00:07-08:00",
          "tree_id": "69d47e4e3b4989c228904304c3cb8ccbbc865e78",
          "url": "https://github.com/hyperium/hyper/commit/af96ddf008540ca4799381efb16a74af4dc3db28"
        },
        "date": 1605571383508,
        "tool": "cargo",
        "benches": [
          {
            "name": "http_connector",
            "value": 43999,
            "range": "± 5706",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "2f2ceb24265a7e63601cf1ffde7d586cd666a783",
          "message": "chore(ci): automatically publish benchmarks in a graph",
          "timestamp": "2020-11-16T16:51:30-08:00",
          "tree_id": "5b90f0b2e08fd62a41b6f1f90d45da63bd559713",
          "url": "https://github.com/hyperium/hyper/commit/2f2ceb24265a7e63601cf1ffde7d586cd666a783"
        },
        "date": 1605574482544,
        "tool": "cargo",
        "benches": [
          {
            "name": "http_connector",
            "value": 69407,
            "range": "± 11740",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "2a19ab74ed69bc776da25544e98979c9fb6e1834",
          "message": "feat(http1): Make HTTP/1 support an optional feature\n\ncc #2251\n\nBREAKING CHANGE: This puts all HTTP/1 methods and support behind an\n  `http1` cargo feature, which will not be enabled by default. To use\n  HTTP/1, add `features = [\"http1\"]` to the hyper dependency in your\n  `Cargo.toml`.",
          "timestamp": "2020-11-17T10:42:20-08:00",
          "tree_id": "9f96eaaa5c228f8eba653b9ef2cfec2d099f3f10",
          "url": "https://github.com/hyperium/hyper/commit/2a19ab74ed69bc776da25544e98979c9fb6e1834"
        },
        "date": 1605638704568,
        "tool": "cargo",
        "benches": [
          {
            "name": "http_connector",
            "value": 38812,
            "range": "± 6297",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "eb092a7b8cdcf16760027522f3ea2e818e138e21",
          "message": "chore(ci): check all feature combinations in CI",
          "timestamp": "2020-11-17T14:30:27-08:00",
          "tree_id": "857a77fb5849465e43aa3949ed1e99e109ca2e95",
          "url": "https://github.com/hyperium/hyper/commit/eb092a7b8cdcf16760027522f3ea2e818e138e21"
        },
        "date": 1605652402414,
        "tool": "cargo",
        "benches": [
          {
            "name": "http_connector",
            "value": 47141,
            "range": "± 3341",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4e55583d30a597884883f1a51b678f5c57c76765",
          "message": "feat(client): Make `client` an optional feature\n\ncc #2223\r\n\r\nBREAKING CHANGE: The HTTP client of hyper is now an optional feature. To\r\n  enable the client, add `features = [\"client\"]` to the dependency in\r\n  your `Cargo.toml`.",
          "timestamp": "2020-11-17T17:06:25-08:00",
          "tree_id": "a96d23e59a63b4783772da0aa92b70f346ba446c",
          "url": "https://github.com/hyperium/hyper/commit/4e55583d30a597884883f1a51b678f5c57c76765"
        },
        "date": 1605661761900,
        "tool": "cargo",
        "benches": [
          {
            "name": "http_connector",
            "value": 51407,
            "range": "± 3443",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bdb5e5d6946f4e3f8115a6b1683aff6a04df73de",
          "message": "feat(server): Make the `server` code an optional feature (#2334)\n\ncc #2223 \r\n\r\nBREAKING CHANGE: The HTTP server code is now an optional feature. To\r\n  enable the server, add `features = [\"server\"]` to the dependency in\r\n  your `Cargo.toml`.",
          "timestamp": "2020-11-18T11:02:20-08:00",
          "tree_id": "260a94fe0611cc0d6d30c331e182fd0bfcc347cf",
          "url": "https://github.com/hyperium/hyper/commit/bdb5e5d6946f4e3f8115a6b1683aff6a04df73de"
        },
        "date": 1605726318291,
        "tool": "cargo",
        "benches": [
          {
            "name": "http_connector",
            "value": 52739,
            "range": "± 1310",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eliza@buoyant.io",
            "name": "Eliza Weisman",
            "username": "hawkw"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "abb6471690f796e1b96bb2d7b1042f424d69f169",
          "message": "refactor(client): use tokio's TcpSocket for more sockopts (#2335)\n\nSigned-off-by: Eliza Weisman <eliza@buoyant.io>",
          "timestamp": "2020-11-18T14:45:45-08:00",
          "tree_id": "c8d1bedaa9af64428ba5cdc93170b1c62cc3564e",
          "url": "https://github.com/hyperium/hyper/commit/abb6471690f796e1b96bb2d7b1042f424d69f169"
        },
        "date": 1605739718567,
        "tool": "cargo",
        "benches": [
          {
            "name": "http_connector",
            "value": 45058,
            "range": "± 5159",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ed2b22a7f66899d338691552fbcb6c0f2f4e06b9",
          "message": "feat(lib): disable all optional features by default (#2336)\n\nBREAKING CHANGE: All optional features have been disabled by default.",
          "timestamp": "2020-11-19T10:05:39-08:00",
          "tree_id": "6e1ed1ba8f1fec285f11643f67ff48ea7e92a9a5",
          "url": "https://github.com/hyperium/hyper/commit/ed2b22a7f66899d338691552fbcb6c0f2f4e06b9"
        },
        "date": 1605809318072,
        "tool": "cargo",
        "benches": [
          {
            "name": "http_connector",
            "value": 45042,
            "range": "± 4086",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eliza@buoyant.io",
            "name": "Eliza Weisman",
            "username": "hawkw"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "751c122589cfd9935e8e3239cd0d692e573784c5",
          "message": "feat(lib): update `bytes` to 0.6, update `http-body` (#2339)\n\nThis branch updates `bytes` and `http-body` to the latest versions. The\r\n`http-body` version that uses `bytes` 0.6 hasn't been released yet, so\r\nwe depend on it via a git dep for now. Presumably Hyper and `http-body`\r\nwill synchronize their releases.\r\n\r\nOther than that, this is a pretty mechanical update. Should fix the\r\nbuild and unblock the `h2` update to use vectored writes.",
          "timestamp": "2020-11-19T16:23:32-08:00",
          "tree_id": "97ddcd8135cc1bf64a6939749febad0836f5276c",
          "url": "https://github.com/hyperium/hyper/commit/751c122589cfd9935e8e3239cd0d692e573784c5"
        },
        "date": 1605831968329,
        "tool": "cargo",
        "benches": [
          {
            "name": "http_connector",
            "value": 50639,
            "range": "± 1184",
            "unit": "ns/iter"
          }
        ]
      }
    ],
    "pipeline": [
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "af96ddf008540ca4799381efb16a74af4dc3db28",
          "message": "Use patched GH action",
          "timestamp": "2020-11-16T16:00:07-08:00",
          "tree_id": "69d47e4e3b4989c228904304c3cb8ccbbc865e78",
          "url": "https://github.com/hyperium/hyper/commit/af96ddf008540ca4799381efb16a74af4dc3db28"
        },
        "date": 1605571384450,
        "tool": "cargo",
        "benches": [
          {
            "name": "hello_world_16",
            "value": 59874,
            "range": "± 41739",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "2f2ceb24265a7e63601cf1ffde7d586cd666a783",
          "message": "chore(ci): automatically publish benchmarks in a graph",
          "timestamp": "2020-11-16T16:51:30-08:00",
          "tree_id": "5b90f0b2e08fd62a41b6f1f90d45da63bd559713",
          "url": "https://github.com/hyperium/hyper/commit/2f2ceb24265a7e63601cf1ffde7d586cd666a783"
        },
        "date": 1605574492714,
        "tool": "cargo",
        "benches": [
          {
            "name": "hello_world_16",
            "value": 72933,
            "range": "± 15934",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "2a19ab74ed69bc776da25544e98979c9fb6e1834",
          "message": "feat(http1): Make HTTP/1 support an optional feature\n\ncc #2251\n\nBREAKING CHANGE: This puts all HTTP/1 methods and support behind an\n  `http1` cargo feature, which will not be enabled by default. To use\n  HTTP/1, add `features = [\"http1\"]` to the hyper dependency in your\n  `Cargo.toml`.",
          "timestamp": "2020-11-17T10:42:20-08:00",
          "tree_id": "9f96eaaa5c228f8eba653b9ef2cfec2d099f3f10",
          "url": "https://github.com/hyperium/hyper/commit/2a19ab74ed69bc776da25544e98979c9fb6e1834"
        },
        "date": 1605638741076,
        "tool": "cargo",
        "benches": [
          {
            "name": "hello_world_16",
            "value": 58120,
            "range": "± 3037",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "eb092a7b8cdcf16760027522f3ea2e818e138e21",
          "message": "chore(ci): check all feature combinations in CI",
          "timestamp": "2020-11-17T14:30:27-08:00",
          "tree_id": "857a77fb5849465e43aa3949ed1e99e109ca2e95",
          "url": "https://github.com/hyperium/hyper/commit/eb092a7b8cdcf16760027522f3ea2e818e138e21"
        },
        "date": 1605652414646,
        "tool": "cargo",
        "benches": [
          {
            "name": "hello_world_16",
            "value": 56748,
            "range": "± 3644",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4e55583d30a597884883f1a51b678f5c57c76765",
          "message": "feat(client): Make `client` an optional feature\n\ncc #2223\r\n\r\nBREAKING CHANGE: The HTTP client of hyper is now an optional feature. To\r\n  enable the client, add `features = [\"client\"]` to the dependency in\r\n  your `Cargo.toml`.",
          "timestamp": "2020-11-17T17:06:25-08:00",
          "tree_id": "a96d23e59a63b4783772da0aa92b70f346ba446c",
          "url": "https://github.com/hyperium/hyper/commit/4e55583d30a597884883f1a51b678f5c57c76765"
        },
        "date": 1605661773401,
        "tool": "cargo",
        "benches": [
          {
            "name": "hello_world_16",
            "value": 58946,
            "range": "± 2463",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bdb5e5d6946f4e3f8115a6b1683aff6a04df73de",
          "message": "feat(server): Make the `server` code an optional feature (#2334)\n\ncc #2223 \r\n\r\nBREAKING CHANGE: The HTTP server code is now an optional feature. To\r\n  enable the server, add `features = [\"server\"]` to the dependency in\r\n  your `Cargo.toml`.",
          "timestamp": "2020-11-18T11:02:20-08:00",
          "tree_id": "260a94fe0611cc0d6d30c331e182fd0bfcc347cf",
          "url": "https://github.com/hyperium/hyper/commit/bdb5e5d6946f4e3f8115a6b1683aff6a04df73de"
        },
        "date": 1605726333089,
        "tool": "cargo",
        "benches": [
          {
            "name": "hello_world_16",
            "value": 58113,
            "range": "± 6540",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eliza@buoyant.io",
            "name": "Eliza Weisman",
            "username": "hawkw"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "abb6471690f796e1b96bb2d7b1042f424d69f169",
          "message": "refactor(client): use tokio's TcpSocket for more sockopts (#2335)\n\nSigned-off-by: Eliza Weisman <eliza@buoyant.io>",
          "timestamp": "2020-11-18T14:45:45-08:00",
          "tree_id": "c8d1bedaa9af64428ba5cdc93170b1c62cc3564e",
          "url": "https://github.com/hyperium/hyper/commit/abb6471690f796e1b96bb2d7b1042f424d69f169"
        },
        "date": 1605739741120,
        "tool": "cargo",
        "benches": [
          {
            "name": "hello_world_16",
            "value": 61873,
            "range": "± 14397",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ed2b22a7f66899d338691552fbcb6c0f2f4e06b9",
          "message": "feat(lib): disable all optional features by default (#2336)\n\nBREAKING CHANGE: All optional features have been disabled by default.",
          "timestamp": "2020-11-19T10:05:39-08:00",
          "tree_id": "6e1ed1ba8f1fec285f11643f67ff48ea7e92a9a5",
          "url": "https://github.com/hyperium/hyper/commit/ed2b22a7f66899d338691552fbcb6c0f2f4e06b9"
        },
        "date": 1605809351375,
        "tool": "cargo",
        "benches": [
          {
            "name": "hello_world_16",
            "value": 68793,
            "range": "± 13015",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eliza@buoyant.io",
            "name": "Eliza Weisman",
            "username": "hawkw"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "751c122589cfd9935e8e3239cd0d692e573784c5",
          "message": "feat(lib): update `bytes` to 0.6, update `http-body` (#2339)\n\nThis branch updates `bytes` and `http-body` to the latest versions. The\r\n`http-body` version that uses `bytes` 0.6 hasn't been released yet, so\r\nwe depend on it via a git dep for now. Presumably Hyper and `http-body`\r\nwill synchronize their releases.\r\n\r\nOther than that, this is a pretty mechanical update. Should fix the\r\nbuild and unblock the `h2` update to use vectored writes.",
          "timestamp": "2020-11-19T16:23:32-08:00",
          "tree_id": "97ddcd8135cc1bf64a6939749febad0836f5276c",
          "url": "https://github.com/hyperium/hyper/commit/751c122589cfd9935e8e3239cd0d692e573784c5"
        },
        "date": 1605831974430,
        "tool": "cargo",
        "benches": [
          {
            "name": "hello_world_16",
            "value": 49395,
            "range": "± 3193",
            "unit": "ns/iter"
          }
        ]
      }
    ],
    "end_to_end": [
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "af96ddf008540ca4799381efb16a74af4dc3db28",
          "message": "Use patched GH action",
          "timestamp": "2020-11-16T16:00:07-08:00",
          "tree_id": "69d47e4e3b4989c228904304c3cb8ccbbc865e78",
          "url": "https://github.com/hyperium/hyper/commit/af96ddf008540ca4799381efb16a74af4dc3db28"
        },
        "date": 1605571590518,
        "tool": "cargo",
        "benches": [
          {
            "name": "http1_body_both_100kb",
            "value": 132722,
            "range": "± 16591",
            "unit": "ns/iter"
          },
          {
            "name": "http1_body_both_10mb",
            "value": 11231679,
            "range": "± 2188075",
            "unit": "ns/iter"
          },
          {
            "name": "http1_get",
            "value": 42847,
            "range": "± 5081",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_empty",
            "value": 253134,
            "range": "± 50058",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10kb_100_chunks",
            "value": 51995669,
            "range": "± 2403504",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10mb",
            "value": 73491330,
            "range": "± 5068683",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_10mb",
            "value": 75099040,
            "range": "± 7369586",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_1mb",
            "value": 5465725,
            "range": "± 1227984",
            "unit": "ns/iter"
          },
          {
            "name": "http1_post",
            "value": 49395,
            "range": "± 8944",
            "unit": "ns/iter"
          },
          {
            "name": "http2_get",
            "value": 81639,
            "range": "± 7400",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_empty",
            "value": 224091,
            "range": "± 37776",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks",
            "value": 12655158,
            "range": "± 1663726",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_adaptive_window",
            "value": 12744688,
            "range": "± 8988030",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_max_window",
            "value": 12288728,
            "range": "± 9026296",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10mb",
            "value": 65842241,
            "range": "± 6875551",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_10mb",
            "value": 66344191,
            "range": "± 6079532",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_1mb",
            "value": 6844583,
            "range": "± 1275315",
            "unit": "ns/iter"
          },
          {
            "name": "http2_post",
            "value": 93307,
            "range": "± 5680",
            "unit": "ns/iter"
          },
          {
            "name": "http2_req_100kb",
            "value": 176099,
            "range": "± 39857",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "2f2ceb24265a7e63601cf1ffde7d586cd666a783",
          "message": "chore(ci): automatically publish benchmarks in a graph",
          "timestamp": "2020-11-16T16:51:30-08:00",
          "tree_id": "5b90f0b2e08fd62a41b6f1f90d45da63bd559713",
          "url": "https://github.com/hyperium/hyper/commit/2f2ceb24265a7e63601cf1ffde7d586cd666a783"
        },
        "date": 1605574650831,
        "tool": "cargo",
        "benches": [
          {
            "name": "http1_body_both_100kb",
            "value": 80607,
            "range": "± 19255",
            "unit": "ns/iter"
          },
          {
            "name": "http1_body_both_10mb",
            "value": 11142748,
            "range": "± 2664775",
            "unit": "ns/iter"
          },
          {
            "name": "http1_get",
            "value": 30511,
            "range": "± 6748",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_empty",
            "value": 211258,
            "range": "± 60035",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10kb_100_chunks",
            "value": 52791874,
            "range": "± 3145769",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10mb",
            "value": 76846497,
            "range": "± 5930335",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_10mb",
            "value": 77577251,
            "range": "± 7428127",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_1mb",
            "value": 5664720,
            "range": "± 1209271",
            "unit": "ns/iter"
          },
          {
            "name": "http1_post",
            "value": 31829,
            "range": "± 4571",
            "unit": "ns/iter"
          },
          {
            "name": "http2_get",
            "value": 54035,
            "range": "± 6809",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_empty",
            "value": 183562,
            "range": "± 17261",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks",
            "value": 9854340,
            "range": "± 1338186",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_adaptive_window",
            "value": 10090010,
            "range": "± 9892214",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_max_window",
            "value": 9809590,
            "range": "± 10610070",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10mb",
            "value": 63177324,
            "range": "± 11005842",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_10mb",
            "value": 63774634,
            "range": "± 10733731",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_1mb",
            "value": 6784835,
            "range": "± 1077822",
            "unit": "ns/iter"
          },
          {
            "name": "http2_post",
            "value": 63108,
            "range": "± 9599",
            "unit": "ns/iter"
          },
          {
            "name": "http2_req_100kb",
            "value": 126309,
            "range": "± 16024",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "2a19ab74ed69bc776da25544e98979c9fb6e1834",
          "message": "feat(http1): Make HTTP/1 support an optional feature\n\ncc #2251\n\nBREAKING CHANGE: This puts all HTTP/1 methods and support behind an\n  `http1` cargo feature, which will not be enabled by default. To use\n  HTTP/1, add `features = [\"http1\"]` to the hyper dependency in your\n  `Cargo.toml`.",
          "timestamp": "2020-11-17T10:42:20-08:00",
          "tree_id": "9f96eaaa5c228f8eba653b9ef2cfec2d099f3f10",
          "url": "https://github.com/hyperium/hyper/commit/2a19ab74ed69bc776da25544e98979c9fb6e1834"
        },
        "date": 1605638942066,
        "tool": "cargo",
        "benches": [
          {
            "name": "http1_body_both_100kb",
            "value": 141011,
            "range": "± 22273",
            "unit": "ns/iter"
          },
          {
            "name": "http1_body_both_10mb",
            "value": 9792586,
            "range": "± 2095655",
            "unit": "ns/iter"
          },
          {
            "name": "http1_get",
            "value": 41166,
            "range": "± 7703",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_empty",
            "value": 254162,
            "range": "± 53833",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10kb_100_chunks",
            "value": 52013207,
            "range": "± 881214",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10mb",
            "value": 84370132,
            "range": "± 8239613",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_10mb",
            "value": 86486654,
            "range": "± 7435911",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_1mb",
            "value": 4452261,
            "range": "± 1124643",
            "unit": "ns/iter"
          },
          {
            "name": "http1_post",
            "value": 43852,
            "range": "± 12136",
            "unit": "ns/iter"
          },
          {
            "name": "http2_get",
            "value": 77958,
            "range": "± 12906",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_empty",
            "value": 200091,
            "range": "± 41814",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks",
            "value": 12067464,
            "range": "± 2187185",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_adaptive_window",
            "value": 12566806,
            "range": "± 9655577",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_max_window",
            "value": 12186264,
            "range": "± 9710276",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10mb",
            "value": 66912837,
            "range": "± 10622283",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_10mb",
            "value": 66833871,
            "range": "± 11492677",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_1mb",
            "value": 6971657,
            "range": "± 1552041",
            "unit": "ns/iter"
          },
          {
            "name": "http2_post",
            "value": 86745,
            "range": "± 16562",
            "unit": "ns/iter"
          },
          {
            "name": "http2_req_100kb",
            "value": 161916,
            "range": "± 32086",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "distinct": true,
          "id": "eb092a7b8cdcf16760027522f3ea2e818e138e21",
          "message": "chore(ci): check all feature combinations in CI",
          "timestamp": "2020-11-17T14:30:27-08:00",
          "tree_id": "857a77fb5849465e43aa3949ed1e99e109ca2e95",
          "url": "https://github.com/hyperium/hyper/commit/eb092a7b8cdcf16760027522f3ea2e818e138e21"
        },
        "date": 1605652579342,
        "tool": "cargo",
        "benches": [
          {
            "name": "http1_body_both_100kb",
            "value": 87006,
            "range": "± 9388",
            "unit": "ns/iter"
          },
          {
            "name": "http1_body_both_10mb",
            "value": 11927923,
            "range": "± 1045584",
            "unit": "ns/iter"
          },
          {
            "name": "http1_get",
            "value": 34657,
            "range": "± 10809",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_empty",
            "value": 248613,
            "range": "± 14724",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10kb_100_chunks",
            "value": 52012130,
            "range": "± 850928",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10mb",
            "value": 76964924,
            "range": "± 2754425",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_10mb",
            "value": 75877759,
            "range": "± 3549474",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_1mb",
            "value": 5573998,
            "range": "± 426065",
            "unit": "ns/iter"
          },
          {
            "name": "http1_post",
            "value": 34558,
            "range": "± 3133",
            "unit": "ns/iter"
          },
          {
            "name": "http2_get",
            "value": 56265,
            "range": "± 4254",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_empty",
            "value": 184110,
            "range": "± 24613",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks",
            "value": 10407407,
            "range": "± 867792",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_adaptive_window",
            "value": 10282145,
            "range": "± 8997840",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_max_window",
            "value": 10141873,
            "range": "± 9428101",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10mb",
            "value": 64344723,
            "range": "± 8247775",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_10mb",
            "value": 65273973,
            "range": "± 11329481",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_1mb",
            "value": 7107217,
            "range": "± 638185",
            "unit": "ns/iter"
          },
          {
            "name": "http2_post",
            "value": 68218,
            "range": "± 2807",
            "unit": "ns/iter"
          },
          {
            "name": "http2_req_100kb",
            "value": 138215,
            "range": "± 4388",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4e55583d30a597884883f1a51b678f5c57c76765",
          "message": "feat(client): Make `client` an optional feature\n\ncc #2223\r\n\r\nBREAKING CHANGE: The HTTP client of hyper is now an optional feature. To\r\n  enable the client, add `features = [\"client\"]` to the dependency in\r\n  your `Cargo.toml`.",
          "timestamp": "2020-11-17T17:06:25-08:00",
          "tree_id": "a96d23e59a63b4783772da0aa92b70f346ba446c",
          "url": "https://github.com/hyperium/hyper/commit/4e55583d30a597884883f1a51b678f5c57c76765"
        },
        "date": 1605661916967,
        "tool": "cargo",
        "benches": [
          {
            "name": "http1_body_both_100kb",
            "value": 87040,
            "range": "± 2206",
            "unit": "ns/iter"
          },
          {
            "name": "http1_body_both_10mb",
            "value": 11155599,
            "range": "± 525128",
            "unit": "ns/iter"
          },
          {
            "name": "http1_get",
            "value": 31607,
            "range": "± 556",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_empty",
            "value": 225219,
            "range": "± 5206",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10kb_100_chunks",
            "value": 52793915,
            "range": "± 843387",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10mb",
            "value": 74861785,
            "range": "± 885355",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_10mb",
            "value": 75130703,
            "range": "± 1026937",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_1mb",
            "value": 5506139,
            "range": "± 569589",
            "unit": "ns/iter"
          },
          {
            "name": "http1_post",
            "value": 35525,
            "range": "± 1108",
            "unit": "ns/iter"
          },
          {
            "name": "http2_get",
            "value": 58395,
            "range": "± 925",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_empty",
            "value": 192208,
            "range": "± 1781",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks",
            "value": 10621795,
            "range": "± 122683",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_adaptive_window",
            "value": 10735915,
            "range": "± 9260329",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_max_window",
            "value": 10346962,
            "range": "± 9759279",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10mb",
            "value": 65951929,
            "range": "± 3913124",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_10mb",
            "value": 69571330,
            "range": "± 5394326",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_1mb",
            "value": 7155803,
            "range": "± 492675",
            "unit": "ns/iter"
          },
          {
            "name": "http2_post",
            "value": 66980,
            "range": "± 2726",
            "unit": "ns/iter"
          },
          {
            "name": "http2_req_100kb",
            "value": 133244,
            "range": "± 4102",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bdb5e5d6946f4e3f8115a6b1683aff6a04df73de",
          "message": "feat(server): Make the `server` code an optional feature (#2334)\n\ncc #2223 \r\n\r\nBREAKING CHANGE: The HTTP server code is now an optional feature. To\r\n  enable the server, add `features = [\"server\"]` to the dependency in\r\n  your `Cargo.toml`.",
          "timestamp": "2020-11-18T11:02:20-08:00",
          "tree_id": "260a94fe0611cc0d6d30c331e182fd0bfcc347cf",
          "url": "https://github.com/hyperium/hyper/commit/bdb5e5d6946f4e3f8115a6b1683aff6a04df73de"
        },
        "date": 1605726481069,
        "tool": "cargo",
        "benches": [
          {
            "name": "http1_body_both_100kb",
            "value": 87540,
            "range": "± 3599",
            "unit": "ns/iter"
          },
          {
            "name": "http1_body_both_10mb",
            "value": 10845586,
            "range": "± 822747",
            "unit": "ns/iter"
          },
          {
            "name": "http1_get",
            "value": 33745,
            "range": "± 1128",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_empty",
            "value": 241590,
            "range": "± 18541",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10kb_100_chunks",
            "value": 51995012,
            "range": "± 109608",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10mb",
            "value": 75007205,
            "range": "± 4107758",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_10mb",
            "value": 74601597,
            "range": "± 3422088",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_1mb",
            "value": 5367724,
            "range": "± 823706",
            "unit": "ns/iter"
          },
          {
            "name": "http1_post",
            "value": 36958,
            "range": "± 1210",
            "unit": "ns/iter"
          },
          {
            "name": "http2_get",
            "value": 59554,
            "range": "± 2904",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_empty",
            "value": 195192,
            "range": "± 3068",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks",
            "value": 10999072,
            "range": "± 409030",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_adaptive_window",
            "value": 11136039,
            "range": "± 8516094",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_max_window",
            "value": 10860245,
            "range": "± 8997839",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10mb",
            "value": 68426704,
            "range": "± 5372140",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_10mb",
            "value": 68119719,
            "range": "± 7583882",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_1mb",
            "value": 7145868,
            "range": "± 499387",
            "unit": "ns/iter"
          },
          {
            "name": "http2_post",
            "value": 69182,
            "range": "± 5606",
            "unit": "ns/iter"
          },
          {
            "name": "http2_req_100kb",
            "value": 141079,
            "range": "± 4696",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eliza@buoyant.io",
            "name": "Eliza Weisman",
            "username": "hawkw"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "abb6471690f796e1b96bb2d7b1042f424d69f169",
          "message": "refactor(client): use tokio's TcpSocket for more sockopts (#2335)\n\nSigned-off-by: Eliza Weisman <eliza@buoyant.io>",
          "timestamp": "2020-11-18T14:45:45-08:00",
          "tree_id": "c8d1bedaa9af64428ba5cdc93170b1c62cc3564e",
          "url": "https://github.com/hyperium/hyper/commit/abb6471690f796e1b96bb2d7b1042f424d69f169"
        },
        "date": 1605739896513,
        "tool": "cargo",
        "benches": [
          {
            "name": "http1_body_both_100kb",
            "value": 121352,
            "range": "± 9351",
            "unit": "ns/iter"
          },
          {
            "name": "http1_body_both_10mb",
            "value": 10947680,
            "range": "± 1276805",
            "unit": "ns/iter"
          },
          {
            "name": "http1_get",
            "value": 41019,
            "range": "± 3522",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_empty",
            "value": 237803,
            "range": "± 65767",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10kb_100_chunks",
            "value": 54353656,
            "range": "± 3997238",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10mb",
            "value": 74063371,
            "range": "± 4939322",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_10mb",
            "value": 73584876,
            "range": "± 3870310",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_1mb",
            "value": 4673657,
            "range": "± 702866",
            "unit": "ns/iter"
          },
          {
            "name": "http1_post",
            "value": 46325,
            "range": "± 13015",
            "unit": "ns/iter"
          },
          {
            "name": "http2_get",
            "value": 78470,
            "range": "± 20131",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_empty",
            "value": 198415,
            "range": "± 19995",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks",
            "value": 11709572,
            "range": "± 1378385",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_adaptive_window",
            "value": 11737294,
            "range": "± 8863450",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_max_window",
            "value": 11424512,
            "range": "± 9279136",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10mb",
            "value": 62352317,
            "range": "± 11648069",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_10mb",
            "value": 63639312,
            "range": "± 7282604",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_1mb",
            "value": 6606716,
            "range": "± 1299923",
            "unit": "ns/iter"
          },
          {
            "name": "http2_post",
            "value": 89586,
            "range": "± 24253",
            "unit": "ns/iter"
          },
          {
            "name": "http2_req_100kb",
            "value": 162348,
            "range": "± 14838",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "sean@seanmonstar.com",
            "name": "Sean McArthur",
            "username": "seanmonstar"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ed2b22a7f66899d338691552fbcb6c0f2f4e06b9",
          "message": "feat(lib): disable all optional features by default (#2336)\n\nBREAKING CHANGE: All optional features have been disabled by default.",
          "timestamp": "2020-11-19T10:05:39-08:00",
          "tree_id": "6e1ed1ba8f1fec285f11643f67ff48ea7e92a9a5",
          "url": "https://github.com/hyperium/hyper/commit/ed2b22a7f66899d338691552fbcb6c0f2f4e06b9"
        },
        "date": 1605809513651,
        "tool": "cargo",
        "benches": [
          {
            "name": "http1_body_both_100kb",
            "value": 130168,
            "range": "± 19637",
            "unit": "ns/iter"
          },
          {
            "name": "http1_body_both_10mb",
            "value": 9344678,
            "range": "± 3182566",
            "unit": "ns/iter"
          },
          {
            "name": "http1_get",
            "value": 37078,
            "range": "± 6317",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_empty",
            "value": 232295,
            "range": "± 29130",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10kb_100_chunks",
            "value": 52004632,
            "range": "± 872862",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10mb",
            "value": 79618993,
            "range": "± 8081246",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_10mb",
            "value": 80550587,
            "range": "± 6901944",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_1mb",
            "value": 4322367,
            "range": "± 1056988",
            "unit": "ns/iter"
          },
          {
            "name": "http1_post",
            "value": 44551,
            "range": "± 8902",
            "unit": "ns/iter"
          },
          {
            "name": "http2_get",
            "value": 73984,
            "range": "± 9312",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_empty",
            "value": 194918,
            "range": "± 36879",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks",
            "value": 11394519,
            "range": "± 1721115",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_adaptive_window",
            "value": 11579119,
            "range": "± 8914019",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_max_window",
            "value": 11320318,
            "range": "± 9398190",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10mb",
            "value": 61835329,
            "range": "± 7704491",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_10mb",
            "value": 62418480,
            "range": "± 8671637",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_1mb",
            "value": 6453074,
            "range": "± 884612",
            "unit": "ns/iter"
          },
          {
            "name": "http2_post",
            "value": 82353,
            "range": "± 16330",
            "unit": "ns/iter"
          },
          {
            "name": "http2_req_100kb",
            "value": 161158,
            "range": "± 26300",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eliza@buoyant.io",
            "name": "Eliza Weisman",
            "username": "hawkw"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "751c122589cfd9935e8e3239cd0d692e573784c5",
          "message": "feat(lib): update `bytes` to 0.6, update `http-body` (#2339)\n\nThis branch updates `bytes` and `http-body` to the latest versions. The\r\n`http-body` version that uses `bytes` 0.6 hasn't been released yet, so\r\nwe depend on it via a git dep for now. Presumably Hyper and `http-body`\r\nwill synchronize their releases.\r\n\r\nOther than that, this is a pretty mechanical update. Should fix the\r\nbuild and unblock the `h2` update to use vectored writes.",
          "timestamp": "2020-11-19T16:23:32-08:00",
          "tree_id": "97ddcd8135cc1bf64a6939749febad0836f5276c",
          "url": "https://github.com/hyperium/hyper/commit/751c122589cfd9935e8e3239cd0d692e573784c5"
        },
        "date": 1605832110751,
        "tool": "cargo",
        "benches": [
          {
            "name": "http1_body_both_100kb",
            "value": 78854,
            "range": "± 869",
            "unit": "ns/iter"
          },
          {
            "name": "http1_body_both_10mb",
            "value": 9834285,
            "range": "± 446986",
            "unit": "ns/iter"
          },
          {
            "name": "http1_get",
            "value": 29628,
            "range": "± 158",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_empty",
            "value": 223135,
            "range": "± 4758",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10kb_100_chunks",
            "value": 51980382,
            "range": "± 935080",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_req_10mb",
            "value": 71272709,
            "range": "± 720560",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_10mb",
            "value": 71949421,
            "range": "± 940046",
            "unit": "ns/iter"
          },
          {
            "name": "http1_parallel_x10_res_1mb",
            "value": 4612197,
            "range": "± 206226",
            "unit": "ns/iter"
          },
          {
            "name": "http1_post",
            "value": 33138,
            "range": "± 296",
            "unit": "ns/iter"
          },
          {
            "name": "http2_get",
            "value": 54451,
            "range": "± 577",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_empty",
            "value": 171307,
            "range": "± 2323",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks",
            "value": 9909168,
            "range": "± 47665",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_adaptive_window",
            "value": 10019587,
            "range": "± 8534479",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10kb_100_chunks_max_window",
            "value": 9697482,
            "range": "± 8885018",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_req_10mb",
            "value": 61271804,
            "range": "± 3565355",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_10mb",
            "value": 61577147,
            "range": "± 4231635",
            "unit": "ns/iter"
          },
          {
            "name": "http2_parallel_x10_res_1mb",
            "value": 6297911,
            "range": "± 363446",
            "unit": "ns/iter"
          },
          {
            "name": "http2_post",
            "value": 61562,
            "range": "± 767",
            "unit": "ns/iter"
          },
          {
            "name": "http2_req_100kb",
            "value": 124945,
            "range": "± 1656",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}