window.BENCHMARK_DATA = {
  "lastUpdate": 1605661775038,
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
      }
    ]
  }
}