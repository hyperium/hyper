//! Status Codes
use std::fmt;
use std::mem::transmute;

// shamelessly lifted from Teepee. I tried a few schemes, this really
// does seem like the best.

/// An HTTP status code (`Status-Code` in RFC 2616).
///
/// This enum is absolutely exhaustive, covering all 500 possible values (100–599).
///
/// For HTTP/2.0, statuses belonging to the 1xx Informational class are invalid.
///
/// As this is a C‐style enum with each variant having a corresponding value, you may use the likes
/// of `Continue as u16` to retreive the value `100u16`. Normally, though, you should not need to do
/// any such thing; just use the status code as a `StatusCode`.
///
/// If you encounter a status code that you do not know how to deal with, you should treat it as the
/// `x00` status code—e.g. for code 123, treat it as 100 (Continue). This can be achieved with
/// `self.class().default_code()`:
///
/// ```rust
/// # use hyper::status::StatusCode::{Code123, Continue};
/// assert_eq!(Code123.class().default_code(), Continue);
/// ```
pub enum StatusCode {
    /// 100 Continue
    Continue = 100,
    /// 101 Switching Protocols
    SwitchingProtocols = 101,
    /// 102 Processing
    Processing = 102,
    /// 103 (unregistered)
    Code103 = 103,
    /// 104 (unregistered)
    Code104 = 104,
    /// 105 (unregistered)
    Code105 = 105,
    /// 106 (unregistered)
    Code106 = 106,
    /// 107 (unregistered)
    Code107 = 107,
    /// 108 (unregistered)
    Code108 = 108,
    /// 109 (unregistered)
    Code109 = 109,
    /// 110 (unregistered)
    Code110 = 110,
    /// 111 (unregistered)
    Code111 = 111,
    /// 112 (unregistered)
    Code112 = 112,
    /// 113 (unregistered)
    Code113 = 113,
    /// 114 (unregistered)
    Code114 = 114,
    /// 115 (unregistered)
    Code115 = 115,
    /// 116 (unregistered)
    Code116 = 116,
    /// 117 (unregistered)
    Code117 = 117,
    /// 118 (unregistered)
    Code118 = 118,
    /// 119 (unregistered)
    Code119 = 119,
    /// 120 (unregistered)
    Code120 = 120,
    /// 121 (unregistered)
    Code121 = 121,
    /// 122 (unregistered)
    Code122 = 122,
    /// 123 (unregistered)
    Code123 = 123,
    /// 124 (unregistered)
    Code124 = 124,
    /// 125 (unregistered)
    Code125 = 125,
    /// 126 (unregistered)
    Code126 = 126,
    /// 127 (unregistered)
    Code127 = 127,
    /// 128 (unregistered)
    Code128 = 128,
    /// 129 (unregistered)
    Code129 = 129,
    /// 130 (unregistered)
    Code130 = 130,
    /// 131 (unregistered)
    Code131 = 131,
    /// 132 (unregistered)
    Code132 = 132,
    /// 133 (unregistered)
    Code133 = 133,
    /// 134 (unregistered)
    Code134 = 134,
    /// 135 (unregistered)
    Code135 = 135,
    /// 136 (unregistered)
    Code136 = 136,
    /// 137 (unregistered)
    Code137 = 137,
    /// 138 (unregistered)
    Code138 = 138,
    /// 139 (unregistered)
    Code139 = 139,
    /// 140 (unregistered)
    Code140 = 140,
    /// 141 (unregistered)
    Code141 = 141,
    /// 142 (unregistered)
    Code142 = 142,
    /// 143 (unregistered)
    Code143 = 143,
    /// 144 (unregistered)
    Code144 = 144,
    /// 145 (unregistered)
    Code145 = 145,
    /// 146 (unregistered)
    Code146 = 146,
    /// 147 (unregistered)
    Code147 = 147,
    /// 148 (unregistered)
    Code148 = 148,
    /// 149 (unregistered)
    Code149 = 149,
    /// 150 (unregistered)
    Code150 = 150,
    /// 151 (unregistered)
    Code151 = 151,
    /// 152 (unregistered)
    Code152 = 152,
    /// 153 (unregistered)
    Code153 = 153,
    /// 154 (unregistered)
    Code154 = 154,
    /// 155 (unregistered)
    Code155 = 155,
    /// 156 (unregistered)
    Code156 = 156,
    /// 157 (unregistered)
    Code157 = 157,
    /// 158 (unregistered)
    Code158 = 158,
    /// 159 (unregistered)
    Code159 = 159,
    /// 160 (unregistered)
    Code160 = 160,
    /// 161 (unregistered)
    Code161 = 161,
    /// 162 (unregistered)
    Code162 = 162,
    /// 163 (unregistered)
    Code163 = 163,
    /// 164 (unregistered)
    Code164 = 164,
    /// 165 (unregistered)
    Code165 = 165,
    /// 166 (unregistered)
    Code166 = 166,
    /// 167 (unregistered)
    Code167 = 167,
    /// 168 (unregistered)
    Code168 = 168,
    /// 169 (unregistered)
    Code169 = 169,
    /// 170 (unregistered)
    Code170 = 170,
    /// 171 (unregistered)
    Code171 = 171,
    /// 172 (unregistered)
    Code172 = 172,
    /// 173 (unregistered)
    Code173 = 173,
    /// 174 (unregistered)
    Code174 = 174,
    /// 175 (unregistered)
    Code175 = 175,
    /// 176 (unregistered)
    Code176 = 176,
    /// 177 (unregistered)
    Code177 = 177,
    /// 178 (unregistered)
    Code178 = 178,
    /// 179 (unregistered)
    Code179 = 179,
    /// 180 (unregistered)
    Code180 = 180,
    /// 181 (unregistered)
    Code181 = 181,
    /// 182 (unregistered)
    Code182 = 182,
    /// 183 (unregistered)
    Code183 = 183,
    /// 184 (unregistered)
    Code184 = 184,
    /// 185 (unregistered)
    Code185 = 185,
    /// 186 (unregistered)
    Code186 = 186,
    /// 187 (unregistered)
    Code187 = 187,
    /// 188 (unregistered)
    Code188 = 188,
    /// 189 (unregistered)
    Code189 = 189,
    /// 190 (unregistered)
    Code190 = 190,
    /// 191 (unregistered)
    Code191 = 191,
    /// 192 (unregistered)
    Code192 = 192,
    /// 193 (unregistered)
    Code193 = 193,
    /// 194 (unregistered)
    Code194 = 194,
    /// 195 (unregistered)
    Code195 = 195,
    /// 196 (unregistered)
    Code196 = 196,
    /// 197 (unregistered)
    Code197 = 197,
    /// 198 (unregistered)
    Code198 = 198,
    /// 199 (unregistered)
    Code199 = 199,

    /// 200 OK
    Ok = 200,
    /// 201 Created
    Created = 201,
    /// 202 Accepted
    Accepted = 202,
    /// 203 Non-Authoritative Information
    NonAuthoritativeInformation = 203,
    /// 204 No Content
    NoContent = 204,
    /// 205 Reset Content
    ResetContent = 205,
    /// 206 Partial Content
    PartialContent = 206,
    /// 207 Multi-Status
    MultiStatus = 207,
    /// 208 Already Reported
    AlreadyReported = 208,
    /// 209 (unregistered)
    Code209 = 209,
    /// 210 (unregistered)
    Code210 = 210,
    /// 211 (unregistered)
    Code211 = 211,
    /// 212 (unregistered)
    Code212 = 212,
    /// 213 (unregistered)
    Code213 = 213,
    /// 214 (unregistered)
    Code214 = 214,
    /// 215 (unregistered)
    Code215 = 215,
    /// 216 (unregistered)
    Code216 = 216,
    /// 217 (unregistered)
    Code217 = 217,
    /// 218 (unregistered)
    Code218 = 218,
    /// 219 (unregistered)
    Code219 = 219,
    /// 220 (unregistered)
    Code220 = 220,
    /// 221 (unregistered)
    Code221 = 221,
    /// 222 (unregistered)
    Code222 = 222,
    /// 223 (unregistered)
    Code223 = 223,
    /// 224 (unregistered)
    Code224 = 224,
    /// 225 (unregistered)
    Code225 = 225,
    /// 226 IM Used
    ImUsed = 226,
    /// 227 (unregistered)
    Code227 = 227,
    /// 228 (unregistered)
    Code228 = 228,
    /// 229 (unregistered)
    Code229 = 229,
    /// 230 (unregistered)
    Code230 = 230,
    /// 231 (unregistered)
    Code231 = 231,
    /// 232 (unregistered)
    Code232 = 232,
    /// 233 (unregistered)
    Code233 = 233,
    /// 234 (unregistered)
    Code234 = 234,
    /// 235 (unregistered)
    Code235 = 235,
    /// 236 (unregistered)
    Code236 = 236,
    /// 237 (unregistered)
    Code237 = 237,
    /// 238 (unregistered)
    Code238 = 238,
    /// 239 (unregistered)
    Code239 = 239,
    /// 240 (unregistered)
    Code240 = 240,
    /// 241 (unregistered)
    Code241 = 241,
    /// 242 (unregistered)
    Code242 = 242,
    /// 243 (unregistered)
    Code243 = 243,
    /// 244 (unregistered)
    Code244 = 244,
    /// 245 (unregistered)
    Code245 = 245,
    /// 246 (unregistered)
    Code246 = 246,
    /// 247 (unregistered)
    Code247 = 247,
    /// 248 (unregistered)
    Code248 = 248,
    /// 249 (unregistered)
    Code249 = 249,
    /// 250 (unregistered)
    Code250 = 250,
    /// 251 (unregistered)
    Code251 = 251,
    /// 252 (unregistered)
    Code252 = 252,
    /// 253 (unregistered)
    Code253 = 253,
    /// 254 (unregistered)
    Code254 = 254,
    /// 255 (unregistered)
    Code255 = 255,
    /// 256 (unregistered)
    Code256 = 256,
    /// 257 (unregistered)
    Code257 = 257,
    /// 258 (unregistered)
    Code258 = 258,
    /// 259 (unregistered)
    Code259 = 259,
    /// 260 (unregistered)
    Code260 = 260,
    /// 261 (unregistered)
    Code261 = 261,
    /// 262 (unregistered)
    Code262 = 262,
    /// 263 (unregistered)
    Code263 = 263,
    /// 264 (unregistered)
    Code264 = 264,
    /// 265 (unregistered)
    Code265 = 265,
    /// 266 (unregistered)
    Code266 = 266,
    /// 267 (unregistered)
    Code267 = 267,
    /// 268 (unregistered)
    Code268 = 268,
    /// 269 (unregistered)
    Code269 = 269,
    /// 270 (unregistered)
    Code270 = 270,
    /// 271 (unregistered)
    Code271 = 271,
    /// 272 (unregistered)
    Code272 = 272,
    /// 273 (unregistered)
    Code273 = 273,
    /// 274 (unregistered)
    Code274 = 274,
    /// 275 (unregistered)
    Code275 = 275,
    /// 276 (unregistered)
    Code276 = 276,
    /// 277 (unregistered)
    Code277 = 277,
    /// 278 (unregistered)
    Code278 = 278,
    /// 279 (unregistered)
    Code279 = 279,
    /// 280 (unregistered)
    Code280 = 280,
    /// 281 (unregistered)
    Code281 = 281,
    /// 282 (unregistered)
    Code282 = 282,
    /// 283 (unregistered)
    Code283 = 283,
    /// 284 (unregistered)
    Code284 = 284,
    /// 285 (unregistered)
    Code285 = 285,
    /// 286 (unregistered)
    Code286 = 286,
    /// 287 (unregistered)
    Code287 = 287,
    /// 288 (unregistered)
    Code288 = 288,
    /// 289 (unregistered)
    Code289 = 289,
    /// 290 (unregistered)
    Code290 = 290,
    /// 291 (unregistered)
    Code291 = 291,
    /// 292 (unregistered)
    Code292 = 292,
    /// 293 (unregistered)
    Code293 = 293,
    /// 294 (unregistered)
    Code294 = 294,
    /// 295 (unregistered)
    Code295 = 295,
    /// 296 (unregistered)
    Code296 = 296,
    /// 297 (unregistered)
    Code297 = 297,
    /// 298 (unregistered)
    Code298 = 298,
    /// 299 (unregistered)
    Code299 = 299,

    /// 300 Multiple Choices
    MultipleChoices = 300,
    /// 301 Moved Permanently
    MovedPermanently = 301,
    /// 302 Found
    Found = 302,
    /// 303 See Other
    SeeOther = 303,
    /// 304 Not Modified
    NotModified = 304,
    /// 305 Use Proxy
    UseProxy = 305,
    /// 306 Switch Proxy
    SwitchProxy = 306,
    /// 307 Temporary Redirect
    TemporaryRedirect = 307,
    /// 308 Permanent Redirect
    PermanentRedirect = 308,
    /// 309 (unregistered)
    Code309 = 309,
    /// 310 (unregistered)
    Code310 = 310,
    /// 311 (unregistered)
    Code311 = 311,
    /// 312 (unregistered)
    Code312 = 312,
    /// 313 (unregistered)
    Code313 = 313,
    /// 314 (unregistered)
    Code314 = 314,
    /// 315 (unregistered)
    Code315 = 315,
    /// 316 (unregistered)
    Code316 = 316,
    /// 317 (unregistered)
    Code317 = 317,
    /// 318 (unregistered)
    Code318 = 318,
    /// 319 (unregistered)
    Code319 = 319,
    /// 320 (unregistered)
    Code320 = 320,
    /// 321 (unregistered)
    Code321 = 321,
    /// 322 (unregistered)
    Code322 = 322,
    /// 323 (unregistered)
    Code323 = 323,
    /// 324 (unregistered)
    Code324 = 324,
    /// 325 (unregistered)
    Code325 = 325,
    /// 326 (unregistered)
    Code326 = 326,
    /// 327 (unregistered)
    Code327 = 327,
    /// 328 (unregistered)
    Code328 = 328,
    /// 329 (unregistered)
    Code329 = 329,
    /// 330 (unregistered)
    Code330 = 330,
    /// 331 (unregistered)
    Code331 = 331,
    /// 332 (unregistered)
    Code332 = 332,
    /// 333 (unregistered)
    Code333 = 333,
    /// 334 (unregistered)
    Code334 = 334,
    /// 335 (unregistered)
    Code335 = 335,
    /// 336 (unregistered)
    Code336 = 336,
    /// 337 (unregistered)
    Code337 = 337,
    /// 338 (unregistered)
    Code338 = 338,
    /// 339 (unregistered)
    Code339 = 339,
    /// 340 (unregistered)
    Code340 = 340,
    /// 341 (unregistered)
    Code341 = 341,
    /// 342 (unregistered)
    Code342 = 342,
    /// 343 (unregistered)
    Code343 = 343,
    /// 344 (unregistered)
    Code344 = 344,
    /// 345 (unregistered)
    Code345 = 345,
    /// 346 (unregistered)
    Code346 = 346,
    /// 347 (unregistered)
    Code347 = 347,
    /// 348 (unregistered)
    Code348 = 348,
    /// 349 (unregistered)
    Code349 = 349,
    /// 350 (unregistered)
    Code350 = 350,
    /// 351 (unregistered)
    Code351 = 351,
    /// 352 (unregistered)
    Code352 = 352,
    /// 353 (unregistered)
    Code353 = 353,
    /// 354 (unregistered)
    Code354 = 354,
    /// 355 (unregistered)
    Code355 = 355,
    /// 356 (unregistered)
    Code356 = 356,
    /// 357 (unregistered)
    Code357 = 357,
    /// 358 (unregistered)
    Code358 = 358,
    /// 359 (unregistered)
    Code359 = 359,
    /// 360 (unregistered)
    Code360 = 360,
    /// 361 (unregistered)
    Code361 = 361,
    /// 362 (unregistered)
    Code362 = 362,
    /// 363 (unregistered)
    Code363 = 363,
    /// 364 (unregistered)
    Code364 = 364,
    /// 365 (unregistered)
    Code365 = 365,
    /// 366 (unregistered)
    Code366 = 366,
    /// 367 (unregistered)
    Code367 = 367,
    /// 368 (unregistered)
    Code368 = 368,
    /// 369 (unregistered)
    Code369 = 369,
    /// 370 (unregistered)
    Code370 = 370,
    /// 371 (unregistered)
    Code371 = 371,
    /// 372 (unregistered)
    Code372 = 372,
    /// 373 (unregistered)
    Code373 = 373,
    /// 374 (unregistered)
    Code374 = 374,
    /// 375 (unregistered)
    Code375 = 375,
    /// 376 (unregistered)
    Code376 = 376,
    /// 377 (unregistered)
    Code377 = 377,
    /// 378 (unregistered)
    Code378 = 378,
    /// 379 (unregistered)
    Code379 = 379,
    /// 380 (unregistered)
    Code380 = 380,
    /// 381 (unregistered)
    Code381 = 381,
    /// 382 (unregistered)
    Code382 = 382,
    /// 383 (unregistered)
    Code383 = 383,
    /// 384 (unregistered)
    Code384 = 384,
    /// 385 (unregistered)
    Code385 = 385,
    /// 386 (unregistered)
    Code386 = 386,
    /// 387 (unregistered)
    Code387 = 387,
    /// 388 (unregistered)
    Code388 = 388,
    /// 389 (unregistered)
    Code389 = 389,
    /// 390 (unregistered)
    Code390 = 390,
    /// 391 (unregistered)
    Code391 = 391,
    /// 392 (unregistered)
    Code392 = 392,
    /// 393 (unregistered)
    Code393 = 393,
    /// 394 (unregistered)
    Code394 = 394,
    /// 395 (unregistered)
    Code395 = 395,
    /// 396 (unregistered)
    Code396 = 396,
    /// 397 (unregistered)
    Code397 = 397,
    /// 398 (unregistered)
    Code398 = 398,
    /// 399 (unregistered)
    Code399 = 399,

    /// 400 Bad Request
    BadRequest = 400,
    /// 401 Unauthorized
    Unauthorized = 401,
    /// 402 Payment Required
    PaymentRequired = 402,
    /// 403 Forbidden
    Forbidden = 403,
    /// 404 Not Found
    NotFound = 404,
    /// 405 Method Not Allowed
    MethodNotAllowed = 405,
    /// 406 Not Acceptable
    NotAcceptable = 406,
    /// 407 Proxy Authentication Required
    ProxyAuthenticationRequired = 407,
    /// 408 Request Timeout
    RequestTimeout = 408,
    /// 409 Conflict
    Conflict = 409,
    /// 410 Gone
    Gone = 410,
    /// 411 Length Required
    LengthRequired = 411,
    /// 412 Precondition Failed
    PreconditionFailed = 412,
    /// 413 Request Entity Too Large
    RequestEntityTooLarge = 413,
    /// 414 Request-URI Too Long
    RequestUriTooLong = 414,
    /// 415 Unsupported Media Type
    UnsupportedMediaType = 415,
    /// 416 Requested Range Not Satisfiable
    RequestedRangeNotSatisfiable = 416,
    /// 417 Expectation Failed
    ExpectationFailed = 417,
    /// 418 I'm a teapot
    ImATeapot = 418,
    /// 419 Authentication Timeout
    AuthenticationTimeout = 419,
    /// 420 (unregistered)
    Code420 = 420,
    /// 421 (unregistered)
    Code421 = 421,
    /// 422 Unprocessable Entity
    UnprocessableEntity = 422,
    /// 423 Locked
    Locked = 423,
    /// 424 Failed Dependency
    FailedDependency = 424,
    /// 425 Unordered Collection
    UnorderedCollection = 425,
    /// 426 Upgrade Required
    UpgradeRequired = 426,
    /// 427 (unregistered)
    Code427 = 427,
    /// 428 Precondition Required
    PreconditionRequired = 428,
    /// 429 Too Many Requests
    TooManyRequests = 429,
    /// 430 (unregistered)
    Code430 = 430,
    /// 431 Request Header Fields Too Large
    RequestHeaderFieldsTooLarge = 431,
    /// 432 (unregistered)
    Code432 = 432,
    /// 433 (unregistered)
    Code433 = 433,
    /// 434 (unregistered)
    Code434 = 434,
    /// 435 (unregistered)
    Code435 = 435,
    /// 436 (unregistered)
    Code436 = 436,
    /// 437 (unregistered)
    Code437 = 437,
    /// 438 (unregistered)
    Code438 = 438,
    /// 439 (unregistered)
    Code439 = 439,
    /// 440 (unregistered)
    Code440 = 440,
    /// 441 (unregistered)
    Code441 = 441,
    /// 442 (unregistered)
    Code442 = 442,
    /// 443 (unregistered)
    Code443 = 443,
    /// 444 (unregistered)
    Code444 = 444,
    /// 445 (unregistered)
    Code445 = 445,
    /// 446 (unregistered)
    Code446 = 446,
    /// 447 (unregistered)
    Code447 = 447,
    /// 448 (unregistered)
    Code448 = 448,
    /// 449 (unregistered)
    Code449 = 449,
    /// 450 (unregistered)
    Code450 = 450,
    /// 451 Unavailable For Legal Reasons
    UnavailableForLegalReasons = 451,
    /// 452 (unregistered)
    Code452 = 452,
    /// 453 (unregistered)
    Code453 = 453,
    /// 454 (unregistered)
    Code454 = 454,
    /// 455 (unregistered)
    Code455 = 455,
    /// 456 (unregistered)
    Code456 = 456,
    /// 457 (unregistered)
    Code457 = 457,
    /// 458 (unregistered)
    Code458 = 458,
    /// 459 (unregistered)
    Code459 = 459,
    /// 460 (unregistered)
    Code460 = 460,
    /// 461 (unregistered)
    Code461 = 461,
    /// 462 (unregistered)
    Code462 = 462,
    /// 463 (unregistered)
    Code463 = 463,
    /// 464 (unregistered)
    Code464 = 464,
    /// 465 (unregistered)
    Code465 = 465,
    /// 466 (unregistered)
    Code466 = 466,
    /// 467 (unregistered)
    Code467 = 467,
    /// 468 (unregistered)
    Code468 = 468,
    /// 469 (unregistered)
    Code469 = 469,
    /// 470 (unregistered)
    Code470 = 470,
    /// 471 (unregistered)
    Code471 = 471,
    /// 472 (unregistered)
    Code472 = 472,
    /// 473 (unregistered)
    Code473 = 473,
    /// 474 (unregistered)
    Code474 = 474,
    /// 475 (unregistered)
    Code475 = 475,
    /// 476 (unregistered)
    Code476 = 476,
    /// 477 (unregistered)
    Code477 = 477,
    /// 478 (unregistered)
    Code478 = 478,
    /// 479 (unregistered)
    Code479 = 479,
    /// 480 (unregistered)
    Code480 = 480,
    /// 481 (unregistered)
    Code481 = 481,
    /// 482 (unregistered)
    Code482 = 482,
    /// 483 (unregistered)
    Code483 = 483,
    /// 484 (unregistered)
    Code484 = 484,
    /// 485 (unregistered)
    Code485 = 485,
    /// 486 (unregistered)
    Code486 = 486,
    /// 487 (unregistered)
    Code487 = 487,
    /// 488 (unregistered)
    Code488 = 488,
    /// 489 (unregistered)
    Code489 = 489,
    /// 490 (unregistered)
    Code490 = 490,
    /// 491 (unregistered)
    Code491 = 491,
    /// 492 (unregistered)
    Code492 = 492,
    /// 493 (unregistered)
    Code493 = 493,
    /// 494 (unregistered)
    Code494 = 494,
    /// 495 (unregistered)
    Code495 = 495,
    /// 496 (unregistered)
    Code496 = 496,
    /// 497 (unregistered)
    Code497 = 497,
    /// 498 (unregistered)
    Code498 = 498,
    /// 499 (unregistered)
    Code499 = 499,

    /// 500 Internal Server Error
    InternalServerError = 500,
    /// 501 Not Implemented
    NotImplemented = 501,
    /// 502 Bad Gateway
    BadGateway = 502,
    /// 503 Service Unavailable
    ServiceUnavailable = 503,
    /// 504 Gateway Timeout
    GatewayTimeout = 504,
    /// 505 HTTP Version Not Supported
    HttpVersionNotSupported = 505,
    /// 506 Variant Also Negotiates
    VariantAlsoNegotiates = 506,
    /// 507 Insufficient Storage
    InsufficientStorage = 507,
    /// 508 Loop Detected
    LoopDetected = 508,
    /// 509 (unregistered)
    Code509 = 509,
    /// 510 Not Extended
    NotExtended = 510,
    /// 511 Network Authentication Required
    NetworkAuthenticationRequired = 511,
    /// 512 (unregistered)
    Code512 = 512,
    /// 513 (unregistered)
    Code513 = 513,
    /// 514 (unregistered)
    Code514 = 514,
    /// 515 (unregistered)
    Code515 = 515,
    /// 516 (unregistered)
    Code516 = 516,
    /// 517 (unregistered)
    Code517 = 517,
    /// 518 (unregistered)
    Code518 = 518,
    /// 519 (unregistered)
    Code519 = 519,
    /// 520 (unregistered)
    Code520 = 520,
    /// 521 (unregistered)
    Code521 = 521,
    /// 522 (unregistered)
    Code522 = 522,
    /// 523 (unregistered)
    Code523 = 523,
    /// 524 (unregistered)
    Code524 = 524,
    /// 525 (unregistered)
    Code525 = 525,
    /// 526 (unregistered)
    Code526 = 526,
    /// 527 (unregistered)
    Code527 = 527,
    /// 528 (unregistered)
    Code528 = 528,
    /// 529 (unregistered)
    Code529 = 529,
    /// 530 (unregistered)
    Code530 = 530,
    /// 531 (unregistered)
    Code531 = 531,
    /// 532 (unregistered)
    Code532 = 532,
    /// 533 (unregistered)
    Code533 = 533,
    /// 534 (unregistered)
    Code534 = 534,
    /// 535 (unregistered)
    Code535 = 535,
    /// 536 (unregistered)
    Code536 = 536,
    /// 537 (unregistered)
    Code537 = 537,
    /// 538 (unregistered)
    Code538 = 538,
    /// 539 (unregistered)
    Code539 = 539,
    /// 540 (unregistered)
    Code540 = 540,
    /// 541 (unregistered)
    Code541 = 541,
    /// 542 (unregistered)
    Code542 = 542,
    /// 543 (unregistered)
    Code543 = 543,
    /// 544 (unregistered)
    Code544 = 544,
    /// 545 (unregistered)
    Code545 = 545,
    /// 546 (unregistered)
    Code546 = 546,
    /// 547 (unregistered)
    Code547 = 547,
    /// 548 (unregistered)
    Code548 = 548,
    /// 549 (unregistered)
    Code549 = 549,
    /// 550 (unregistered)
    Code550 = 550,
    /// 551 (unregistered)
    Code551 = 551,
    /// 552 (unregistered)
    Code552 = 552,
    /// 553 (unregistered)
    Code553 = 553,
    /// 554 (unregistered)
    Code554 = 554,
    /// 555 (unregistered)
    Code555 = 555,
    /// 556 (unregistered)
    Code556 = 556,
    /// 557 (unregistered)
    Code557 = 557,
    /// 558 (unregistered)
    Code558 = 558,
    /// 559 (unregistered)
    Code559 = 559,
    /// 560 (unregistered)
    Code560 = 560,
    /// 561 (unregistered)
    Code561 = 561,
    /// 562 (unregistered)
    Code562 = 562,
    /// 563 (unregistered)
    Code563 = 563,
    /// 564 (unregistered)
    Code564 = 564,
    /// 565 (unregistered)
    Code565 = 565,
    /// 566 (unregistered)
    Code566 = 566,
    /// 567 (unregistered)
    Code567 = 567,
    /// 568 (unregistered)
    Code568 = 568,
    /// 569 (unregistered)
    Code569 = 569,
    /// 570 (unregistered)
    Code570 = 570,
    /// 571 (unregistered)
    Code571 = 571,
    /// 572 (unregistered)
    Code572 = 572,
    /// 573 (unregistered)
    Code573 = 573,
    /// 574 (unregistered)
    Code574 = 574,
    /// 575 (unregistered)
    Code575 = 575,
    /// 576 (unregistered)
    Code576 = 576,
    /// 577 (unregistered)
    Code577 = 577,
    /// 578 (unregistered)
    Code578 = 578,
    /// 579 (unregistered)
    Code579 = 579,
    /// 580 (unregistered)
    Code580 = 580,
    /// 581 (unregistered)
    Code581 = 581,
    /// 582 (unregistered)
    Code582 = 582,
    /// 583 (unregistered)
    Code583 = 583,
    /// 584 (unregistered)
    Code584 = 584,
    /// 585 (unregistered)
    Code585 = 585,
    /// 586 (unregistered)
    Code586 = 586,
    /// 587 (unregistered)
    Code587 = 587,
    /// 588 (unregistered)
    Code588 = 588,
    /// 589 (unregistered)
    Code589 = 589,
    /// 590 (unregistered)
    Code590 = 590,
    /// 591 (unregistered)
    Code591 = 591,
    /// 592 (unregistered)
    Code592 = 592,
    /// 593 (unregistered)
    Code593 = 593,
    /// 594 (unregistered)
    Code594 = 594,
    /// 595 (unregistered)
    Code595 = 595,
    /// 596 (unregistered)
    Code596 = 596,
    /// 597 (unregistered)
    Code597 = 597,
    /// 598 (unregistered)
    Code598 = 598,
    /// 599 (unregistered)
    Code599 = 599,
}

impl StatusCode {

    /// Get the standardised `Reason-Phrase` for this status code.
    ///
    /// This is mostly here for servers writing responses, but could potentially have application at
    /// other times.
    ///
    /// The reason phrase is defined as being exclusively for human readers. You should avoid
    /// derive any meaning from it at all costs.
    ///
    /// Bear in mind also that in HTTP/2.0 the reason phrase is abolished from transmission, and so
    /// this canonical reason phrase really is the only reason phrase you’ll find.
    pub fn canonical_reason(&self) -> Option<&'static str> {
        match *self {
            StatusCode::Continue => Some("Continue"),
            StatusCode::SwitchingProtocols => Some("Switching Protocols"),
            StatusCode::Processing => Some("Processing"),
            StatusCode::Code103 => None,
            StatusCode::Code104 => None,
            StatusCode::Code105 => None,
            StatusCode::Code106 => None,
            StatusCode::Code107 => None,
            StatusCode::Code108 => None,
            StatusCode::Code109 => None,
            StatusCode::Code110 => None,
            StatusCode::Code111 => None,
            StatusCode::Code112 => None,
            StatusCode::Code113 => None,
            StatusCode::Code114 => None,
            StatusCode::Code115 => None,
            StatusCode::Code116 => None,
            StatusCode::Code117 => None,
            StatusCode::Code118 => None,
            StatusCode::Code119 => None,
            StatusCode::Code120 => None,
            StatusCode::Code121 => None,
            StatusCode::Code122 => None,
            StatusCode::Code123 => None,
            StatusCode::Code124 => None,
            StatusCode::Code125 => None,
            StatusCode::Code126 => None,
            StatusCode::Code127 => None,
            StatusCode::Code128 => None,
            StatusCode::Code129 => None,
            StatusCode::Code130 => None,
            StatusCode::Code131 => None,
            StatusCode::Code132 => None,
            StatusCode::Code133 => None,
            StatusCode::Code134 => None,
            StatusCode::Code135 => None,
            StatusCode::Code136 => None,
            StatusCode::Code137 => None,
            StatusCode::Code138 => None,
            StatusCode::Code139 => None,
            StatusCode::Code140 => None,
            StatusCode::Code141 => None,
            StatusCode::Code142 => None,
            StatusCode::Code143 => None,
            StatusCode::Code144 => None,
            StatusCode::Code145 => None,
            StatusCode::Code146 => None,
            StatusCode::Code147 => None,
            StatusCode::Code148 => None,
            StatusCode::Code149 => None,
            StatusCode::Code150 => None,
            StatusCode::Code151 => None,
            StatusCode::Code152 => None,
            StatusCode::Code153 => None,
            StatusCode::Code154 => None,
            StatusCode::Code155 => None,
            StatusCode::Code156 => None,
            StatusCode::Code157 => None,
            StatusCode::Code158 => None,
            StatusCode::Code159 => None,
            StatusCode::Code160 => None,
            StatusCode::Code161 => None,
            StatusCode::Code162 => None,
            StatusCode::Code163 => None,
            StatusCode::Code164 => None,
            StatusCode::Code165 => None,
            StatusCode::Code166 => None,
            StatusCode::Code167 => None,
            StatusCode::Code168 => None,
            StatusCode::Code169 => None,
            StatusCode::Code170 => None,
            StatusCode::Code171 => None,
            StatusCode::Code172 => None,
            StatusCode::Code173 => None,
            StatusCode::Code174 => None,
            StatusCode::Code175 => None,
            StatusCode::Code176 => None,
            StatusCode::Code177 => None,
            StatusCode::Code178 => None,
            StatusCode::Code179 => None,
            StatusCode::Code180 => None,
            StatusCode::Code181 => None,
            StatusCode::Code182 => None,
            StatusCode::Code183 => None,
            StatusCode::Code184 => None,
            StatusCode::Code185 => None,
            StatusCode::Code186 => None,
            StatusCode::Code187 => None,
            StatusCode::Code188 => None,
            StatusCode::Code189 => None,
            StatusCode::Code190 => None,
            StatusCode::Code191 => None,
            StatusCode::Code192 => None,
            StatusCode::Code193 => None,
            StatusCode::Code194 => None,
            StatusCode::Code195 => None,
            StatusCode::Code196 => None,
            StatusCode::Code197 => None,
            StatusCode::Code198 => None,
            StatusCode::Code199 => None,

            StatusCode::Ok => Some("OK"),
            StatusCode::Created => Some("Created"),
            StatusCode::Accepted => Some("Accepted"),
            StatusCode::NonAuthoritativeInformation => Some("Non-Authoritative Information"),
            StatusCode::NoContent => Some("No Content"),
            StatusCode::ResetContent => Some("Reset Content"),
            StatusCode::PartialContent => Some("Partial Content"),
            StatusCode::MultiStatus => Some("Multi-Status"),
            StatusCode::AlreadyReported => Some("Already Reported"),
            StatusCode::Code209 => None,
            StatusCode::Code210 => None,
            StatusCode::Code211 => None,
            StatusCode::Code212 => None,
            StatusCode::Code213 => None,
            StatusCode::Code214 => None,
            StatusCode::Code215 => None,
            StatusCode::Code216 => None,
            StatusCode::Code217 => None,
            StatusCode::Code218 => None,
            StatusCode::Code219 => None,
            StatusCode::Code220 => None,
            StatusCode::Code221 => None,
            StatusCode::Code222 => None,
            StatusCode::Code223 => None,
            StatusCode::Code224 => None,
            StatusCode::Code225 => None,
            StatusCode::ImUsed => Some("IM Used"),
            StatusCode::Code227 => None,
            StatusCode::Code228 => None,
            StatusCode::Code229 => None,
            StatusCode::Code230 => None,
            StatusCode::Code231 => None,
            StatusCode::Code232 => None,
            StatusCode::Code233 => None,
            StatusCode::Code234 => None,
            StatusCode::Code235 => None,
            StatusCode::Code236 => None,
            StatusCode::Code237 => None,
            StatusCode::Code238 => None,
            StatusCode::Code239 => None,
            StatusCode::Code240 => None,
            StatusCode::Code241 => None,
            StatusCode::Code242 => None,
            StatusCode::Code243 => None,
            StatusCode::Code244 => None,
            StatusCode::Code245 => None,
            StatusCode::Code246 => None,
            StatusCode::Code247 => None,
            StatusCode::Code248 => None,
            StatusCode::Code249 => None,
            StatusCode::Code250 => None,
            StatusCode::Code251 => None,
            StatusCode::Code252 => None,
            StatusCode::Code253 => None,
            StatusCode::Code254 => None,
            StatusCode::Code255 => None,
            StatusCode::Code256 => None,
            StatusCode::Code257 => None,
            StatusCode::Code258 => None,
            StatusCode::Code259 => None,
            StatusCode::Code260 => None,
            StatusCode::Code261 => None,
            StatusCode::Code262 => None,
            StatusCode::Code263 => None,
            StatusCode::Code264 => None,
            StatusCode::Code265 => None,
            StatusCode::Code266 => None,
            StatusCode::Code267 => None,
            StatusCode::Code268 => None,
            StatusCode::Code269 => None,
            StatusCode::Code270 => None,
            StatusCode::Code271 => None,
            StatusCode::Code272 => None,
            StatusCode::Code273 => None,
            StatusCode::Code274 => None,
            StatusCode::Code275 => None,
            StatusCode::Code276 => None,
            StatusCode::Code277 => None,
            StatusCode::Code278 => None,
            StatusCode::Code279 => None,
            StatusCode::Code280 => None,
            StatusCode::Code281 => None,
            StatusCode::Code282 => None,
            StatusCode::Code283 => None,
            StatusCode::Code284 => None,
            StatusCode::Code285 => None,
            StatusCode::Code286 => None,
            StatusCode::Code287 => None,
            StatusCode::Code288 => None,
            StatusCode::Code289 => None,
            StatusCode::Code290 => None,
            StatusCode::Code291 => None,
            StatusCode::Code292 => None,
            StatusCode::Code293 => None,
            StatusCode::Code294 => None,
            StatusCode::Code295 => None,
            StatusCode::Code296 => None,
            StatusCode::Code297 => None,
            StatusCode::Code298 => None,
            StatusCode::Code299 => None,

            StatusCode::MultipleChoices => Some("Multiple Choices"),
            StatusCode::MovedPermanently => Some("Moved Permanently"),
            StatusCode::Found => Some("Found"),
            StatusCode::SeeOther => Some("See Other"),
            StatusCode::NotModified => Some("Not Modified"),
            StatusCode::UseProxy => Some("Use Proxy"),
            StatusCode::SwitchProxy => Some("Switch Proxy"),
            StatusCode::TemporaryRedirect => Some("Temporary Redirect"),
            StatusCode::PermanentRedirect => Some("Permanent Redirect"),
            StatusCode::Code309 => None,
            StatusCode::Code310 => None,
            StatusCode::Code311 => None,
            StatusCode::Code312 => None,
            StatusCode::Code313 => None,
            StatusCode::Code314 => None,
            StatusCode::Code315 => None,
            StatusCode::Code316 => None,
            StatusCode::Code317 => None,
            StatusCode::Code318 => None,
            StatusCode::Code319 => None,
            StatusCode::Code320 => None,
            StatusCode::Code321 => None,
            StatusCode::Code322 => None,
            StatusCode::Code323 => None,
            StatusCode::Code324 => None,
            StatusCode::Code325 => None,
            StatusCode::Code326 => None,
            StatusCode::Code327 => None,
            StatusCode::Code328 => None,
            StatusCode::Code329 => None,
            StatusCode::Code330 => None,
            StatusCode::Code331 => None,
            StatusCode::Code332 => None,
            StatusCode::Code333 => None,
            StatusCode::Code334 => None,
            StatusCode::Code335 => None,
            StatusCode::Code336 => None,
            StatusCode::Code337 => None,
            StatusCode::Code338 => None,
            StatusCode::Code339 => None,
            StatusCode::Code340 => None,
            StatusCode::Code341 => None,
            StatusCode::Code342 => None,
            StatusCode::Code343 => None,
            StatusCode::Code344 => None,
            StatusCode::Code345 => None,
            StatusCode::Code346 => None,
            StatusCode::Code347 => None,
            StatusCode::Code348 => None,
            StatusCode::Code349 => None,
            StatusCode::Code350 => None,
            StatusCode::Code351 => None,
            StatusCode::Code352 => None,
            StatusCode::Code353 => None,
            StatusCode::Code354 => None,
            StatusCode::Code355 => None,
            StatusCode::Code356 => None,
            StatusCode::Code357 => None,
            StatusCode::Code358 => None,
            StatusCode::Code359 => None,
            StatusCode::Code360 => None,
            StatusCode::Code361 => None,
            StatusCode::Code362 => None,
            StatusCode::Code363 => None,
            StatusCode::Code364 => None,
            StatusCode::Code365 => None,
            StatusCode::Code366 => None,
            StatusCode::Code367 => None,
            StatusCode::Code368 => None,
            StatusCode::Code369 => None,
            StatusCode::Code370 => None,
            StatusCode::Code371 => None,
            StatusCode::Code372 => None,
            StatusCode::Code373 => None,
            StatusCode::Code374 => None,
            StatusCode::Code375 => None,
            StatusCode::Code376 => None,
            StatusCode::Code377 => None,
            StatusCode::Code378 => None,
            StatusCode::Code379 => None,
            StatusCode::Code380 => None,
            StatusCode::Code381 => None,
            StatusCode::Code382 => None,
            StatusCode::Code383 => None,
            StatusCode::Code384 => None,
            StatusCode::Code385 => None,
            StatusCode::Code386 => None,
            StatusCode::Code387 => None,
            StatusCode::Code388 => None,
            StatusCode::Code389 => None,
            StatusCode::Code390 => None,
            StatusCode::Code391 => None,
            StatusCode::Code392 => None,
            StatusCode::Code393 => None,
            StatusCode::Code394 => None,
            StatusCode::Code395 => None,
            StatusCode::Code396 => None,
            StatusCode::Code397 => None,
            StatusCode::Code398 => None,
            StatusCode::Code399 => None,

            StatusCode::BadRequest => Some("Bad Request"),
            StatusCode::Unauthorized => Some("Unauthorized"),
            StatusCode::PaymentRequired => Some("Payment Required"),
            StatusCode::Forbidden => Some("Forbidden"),
            StatusCode::NotFound => Some("Not Found"),
            StatusCode::MethodNotAllowed => Some("Method Not Allowed"),
            StatusCode::NotAcceptable => Some("Not Acceptable"),
            StatusCode::ProxyAuthenticationRequired => Some("Proxy Authentication Required"),
            StatusCode::RequestTimeout => Some("Request Timeout"),
            StatusCode::Conflict => Some("Conflict"),
            StatusCode::Gone => Some("Gone"),
            StatusCode::LengthRequired => Some("Length Required"),
            StatusCode::PreconditionFailed => Some("Precondition Failed"),
            StatusCode::RequestEntityTooLarge => Some("Request Entity Too Large"),
            StatusCode::RequestUriTooLong => Some("Request-URI Too Long"),
            StatusCode::UnsupportedMediaType => Some("Unsupported Media Type"),
            StatusCode::RequestedRangeNotSatisfiable => Some("Requested Range Not Satisfiable"),
            StatusCode::ExpectationFailed => Some("Expectation Failed"),
            StatusCode::ImATeapot => Some("I'm a teapot"),
            StatusCode::AuthenticationTimeout => Some("Authentication Timeout"),
            StatusCode::Code420 => None,
            StatusCode::Code421 => None,
            StatusCode::UnprocessableEntity => Some("Unprocessable Entity"),
            StatusCode::Locked => Some("Locked"),
            StatusCode::FailedDependency => Some("Failed Dependency"),
            StatusCode::UnorderedCollection => Some("Unordered Collection"),
            StatusCode::UpgradeRequired => Some("Upgrade Required"),
            StatusCode::Code427 => None,
            StatusCode::PreconditionRequired => Some("Precondition Required"),
            StatusCode::TooManyRequests => Some("Too Many Requests"),
            StatusCode::Code430 => None,
            StatusCode::RequestHeaderFieldsTooLarge => Some("Request Header Fields Too Large"),
            StatusCode::Code432 => None,
            StatusCode::Code433 => None,
            StatusCode::Code434 => None,
            StatusCode::Code435 => None,
            StatusCode::Code436 => None,
            StatusCode::Code437 => None,
            StatusCode::Code438 => None,
            StatusCode::Code439 => None,
            StatusCode::Code440 => None,
            StatusCode::Code441 => None,
            StatusCode::Code442 => None,
            StatusCode::Code443 => None,
            StatusCode::Code444 => None,
            StatusCode::Code445 => None,
            StatusCode::Code446 => None,
            StatusCode::Code447 => None,
            StatusCode::Code448 => None,
            StatusCode::Code449 => None,
            StatusCode::Code450 => None,
            StatusCode::UnavailableForLegalReasons => Some("Unavailable For Legal Reasons"),
            StatusCode::Code452 => None,
            StatusCode::Code453 => None,
            StatusCode::Code454 => None,
            StatusCode::Code455 => None,
            StatusCode::Code456 => None,
            StatusCode::Code457 => None,
            StatusCode::Code458 => None,
            StatusCode::Code459 => None,
            StatusCode::Code460 => None,
            StatusCode::Code461 => None,
            StatusCode::Code462 => None,
            StatusCode::Code463 => None,
            StatusCode::Code464 => None,
            StatusCode::Code465 => None,
            StatusCode::Code466 => None,
            StatusCode::Code467 => None,
            StatusCode::Code468 => None,
            StatusCode::Code469 => None,
            StatusCode::Code470 => None,
            StatusCode::Code471 => None,
            StatusCode::Code472 => None,
            StatusCode::Code473 => None,
            StatusCode::Code474 => None,
            StatusCode::Code475 => None,
            StatusCode::Code476 => None,
            StatusCode::Code477 => None,
            StatusCode::Code478 => None,
            StatusCode::Code479 => None,
            StatusCode::Code480 => None,
            StatusCode::Code481 => None,
            StatusCode::Code482 => None,
            StatusCode::Code483 => None,
            StatusCode::Code484 => None,
            StatusCode::Code485 => None,
            StatusCode::Code486 => None,
            StatusCode::Code487 => None,
            StatusCode::Code488 => None,
            StatusCode::Code489 => None,
            StatusCode::Code490 => None,
            StatusCode::Code491 => None,
            StatusCode::Code492 => None,
            StatusCode::Code493 => None,
            StatusCode::Code494 => None,
            StatusCode::Code495 => None,
            StatusCode::Code496 => None,
            StatusCode::Code497 => None,
            StatusCode::Code498 => None,
            StatusCode::Code499 => None,

            StatusCode::InternalServerError => Some("Internal Server Error"),
            StatusCode::NotImplemented => Some("Not Implemented"),
            StatusCode::BadGateway => Some("Bad Gateway"),
            StatusCode::ServiceUnavailable => Some("Service Unavailable"),
            StatusCode::GatewayTimeout => Some("Gateway Timeout"),
            StatusCode::HttpVersionNotSupported => Some("HTTP Version Not Supported"),
            StatusCode::VariantAlsoNegotiates => Some("Variant Also Negotiates"),
            StatusCode::InsufficientStorage => Some("Insufficient Storage"),
            StatusCode::LoopDetected => Some("Loop Detected"),
            StatusCode::Code509 => None,
            StatusCode::NotExtended => Some("Not Extended"),
            StatusCode::NetworkAuthenticationRequired => Some("Network Authentication Required"),
            StatusCode::Code512 => None,
            StatusCode::Code513 => None,
            StatusCode::Code514 => None,
            StatusCode::Code515 => None,
            StatusCode::Code516 => None,
            StatusCode::Code517 => None,
            StatusCode::Code518 => None,
            StatusCode::Code519 => None,
            StatusCode::Code520 => None,
            StatusCode::Code521 => None,
            StatusCode::Code522 => None,
            StatusCode::Code523 => None,
            StatusCode::Code524 => None,
            StatusCode::Code525 => None,
            StatusCode::Code526 => None,
            StatusCode::Code527 => None,
            StatusCode::Code528 => None,
            StatusCode::Code529 => None,
            StatusCode::Code530 => None,
            StatusCode::Code531 => None,
            StatusCode::Code532 => None,
            StatusCode::Code533 => None,
            StatusCode::Code534 => None,
            StatusCode::Code535 => None,
            StatusCode::Code536 => None,
            StatusCode::Code537 => None,
            StatusCode::Code538 => None,
            StatusCode::Code539 => None,
            StatusCode::Code540 => None,
            StatusCode::Code541 => None,
            StatusCode::Code542 => None,
            StatusCode::Code543 => None,
            StatusCode::Code544 => None,
            StatusCode::Code545 => None,
            StatusCode::Code546 => None,
            StatusCode::Code547 => None,
            StatusCode::Code548 => None,
            StatusCode::Code549 => None,
            StatusCode::Code550 => None,
            StatusCode::Code551 => None,
            StatusCode::Code552 => None,
            StatusCode::Code553 => None,
            StatusCode::Code554 => None,
            StatusCode::Code555 => None,
            StatusCode::Code556 => None,
            StatusCode::Code557 => None,
            StatusCode::Code558 => None,
            StatusCode::Code559 => None,
            StatusCode::Code560 => None,
            StatusCode::Code561 => None,
            StatusCode::Code562 => None,
            StatusCode::Code563 => None,
            StatusCode::Code564 => None,
            StatusCode::Code565 => None,
            StatusCode::Code566 => None,
            StatusCode::Code567 => None,
            StatusCode::Code568 => None,
            StatusCode::Code569 => None,
            StatusCode::Code570 => None,
            StatusCode::Code571 => None,
            StatusCode::Code572 => None,
            StatusCode::Code573 => None,
            StatusCode::Code574 => None,
            StatusCode::Code575 => None,
            StatusCode::Code576 => None,
            StatusCode::Code577 => None,
            StatusCode::Code578 => None,
            StatusCode::Code579 => None,
            StatusCode::Code580 => None,
            StatusCode::Code581 => None,
            StatusCode::Code582 => None,
            StatusCode::Code583 => None,
            StatusCode::Code584 => None,
            StatusCode::Code585 => None,
            StatusCode::Code586 => None,
            StatusCode::Code587 => None,
            StatusCode::Code588 => None,
            StatusCode::Code589 => None,
            StatusCode::Code590 => None,
            StatusCode::Code591 => None,
            StatusCode::Code592 => None,
            StatusCode::Code593 => None,
            StatusCode::Code594 => None,
            StatusCode::Code595 => None,
            StatusCode::Code596 => None,
            StatusCode::Code597 => None,
            StatusCode::Code598 => None,
            StatusCode::Code599 => None,
        }
    }

    /// Determine the class of a status code, based on its first digit.
    pub fn class(&self) -> StatusClass {
        let code = *self as u16;  // Range of possible values: 100..599.
        // We could match 100..199 &c., but this way we avoid unreachable!() at the end.
        if code < 200 {
            StatusClass::Informational
        } else if code < 300 {
            StatusClass::Success
        } else if code < 400 {
            StatusClass::Redirection
        } else if code < 500 {
            StatusClass::ClientError
        } else {
            StatusClass::ServerError
        }
    }
}

impl Copy for StatusCode {}

/// Formats the status code, *including* the canonical reason.
///
/// ```rust
/// # use hyper::status::StatusCode::{ImATeapot, Code123};
/// assert_eq!(format!("{}", ImATeapot).as_slice(),
///            "418 I'm a teapot");
/// assert_eq!(format!("{}", Code123).as_slice(),
///            "123 <unknown status code>");
/// ```
///
/// If you wish to just include the number, cast to a u16 instead.
impl fmt::Show for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", *self as u16,
               self.canonical_reason().unwrap_or("<unknown status code>"))
    }
}

// Specified manually because the codegen for derived is slow (at the time of writing on the machine
// of writing, 1.2 seconds) and verbose (though the optimiser cuts it down to size).
impl PartialEq for StatusCode {
    #[inline]
    fn eq(&self, other: &StatusCode) -> bool {
        *self as u16 == *other as u16
    }
}

impl Eq for StatusCode {}

// Ditto (though #[derive(Clone)] only takes about 0.4 seconds).
impl Clone for StatusCode {
    #[inline]
    fn clone(&self) -> StatusCode {
        *self
    }
}

// Of the other common derivable traits, I didn’t measure them, but I guess they would be slow too.

impl FromPrimitive for StatusCode {
    fn from_i64(n: i64) -> Option<StatusCode> {
        if n < 100 || n > 599 {
            None
        } else {
            Some(unsafe { transmute::<u16, StatusCode>(n as u16) })
        }
    }

    fn from_u64(n: u64) -> Option<StatusCode> {
        if n < 100 || n > 599 {
            None
        } else {
            Some(unsafe { transmute::<u16, StatusCode>(n as u16) })
        }
    }
}

impl PartialOrd for StatusCode {
    #[inline]
    fn partial_cmp(&self, other: &StatusCode) -> Option<Ordering> {
        (*self as u16).partial_cmp(&(*other as u16))
    }
}

impl Ord for StatusCode {
    #[inline]
    fn cmp(&self, other: &StatusCode) -> Ordering {
        if *self < *other {
            Less
        } else if *self > *other {
            Greater
        } else {
            Equal
        }
    }
}

impl ToPrimitive for StatusCode {
    fn to_i64(&self) -> Option<i64> {
        Some(*self as i64)
    }

    fn to_u64(&self) -> Option<u64> {
        Some(*self as u64)
    }
}

/// The class of an HTTP `Status-Code`.
///
/// [RFC 2616, section 6.1.1 (Status Code and Reason
/// Phrase)](https://tools.ietf.org/html/rfc2616#section-6.1.1):
///
/// > The first digit of the Status-Code defines the class of response. The
/// > last two digits do not have any categorization role.
/// >
/// > ...
/// >
/// > HTTP status codes are extensible. HTTP applications are not required
/// > to understand the meaning of all registered status codes, though such
/// > understanding is obviously desirable. However, applications MUST
/// > understand the class of any status code, as indicated by the first
/// > digit, and treat any unrecognized response as being equivalent to the
/// > x00 status code of that class, with the exception that an
/// > unrecognized response MUST NOT be cached. For example, if an
/// > unrecognized status code of 431 is received by the client, it can
/// > safely assume that there was something wrong with its request and
/// > treat the response as if it had received a 400 status code. In such
/// > cases, user agents SHOULD present to the user the entity returned
/// > with the response, since that entity is likely to include human-
/// > readable information which will explain the unusual status.
///
/// This can be used in cases where a status code’s meaning is unknown, also,
/// to get the appropriate *category* of status.
///
/// For HTTP/2.0, the 1xx Informational class is invalid.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub enum StatusClass {
    /// 1xx: Informational - Request received, continuing process
    Informational = 100,

    /// 2xx: Success - The action was successfully received, understood, and accepted
    Success = 200,

    /// 3xx: Redirection - Further action must be taken in order to complete the request
    Redirection = 300,

    /// 4xx: Client Error - The request contains bad syntax or cannot be fulfilled
    ClientError = 400,

    /// 5xx: Server Error - The server failed to fulfill an apparently valid request
    ServerError = 500,
}

impl StatusClass {
    /// Get the default status code for the class.
    ///
    /// This produces the x00 status code; thus, for `ClientError` (4xx), for example, this will
    /// produce `BadRequest` (400):
    ///
    /// ```rust
    /// # use hyper::status::StatusClass::ClientError;
    /// # use hyper::status::StatusCode::BadRequest;
    /// assert_eq!(ClientError.default_code(), BadRequest);
    /// ```
    ///
    /// The use for this is outlined in [RFC 2616, section 6.1.1 (Status Code and Reason
    /// Phrase)](https://tools.ietf.org/html/rfc2616#section-6.1.1):
    ///
    /// > HTTP status codes are extensible. HTTP applications are not required
    /// > to understand the meaning of all registered status codes, though such
    /// > understanding is obviously desirable. However, applications MUST
    /// > understand the class of any status code, as indicated by the first
    /// > digit, and treat any unrecognized response as being equivalent to the
    /// > x00 status code of that class, with the exception that an
    /// > unrecognized response MUST NOT be cached. For example, if an
    /// > unrecognized status code of 431 is received by the client, it can
    /// > safely assume that there was something wrong with its request and
    /// > treat the response as if it had received a 400 status code. In such
    /// > cases, user agents SHOULD present to the user the entity returned
    /// > with the response, since that entity is likely to include human-
    /// > readable information which will explain the unusual status.
    ///
    /// This is demonstrated thusly (I’ll use 432 rather than 431 as 431 *is* now in use):
    ///
    /// ```rust
    /// # use hyper::status::StatusCode::{Code432, BadRequest};
    /// // Suppose we have received this status code.
    /// let status = Code432;
    ///
    /// // Uh oh! Don’t know what to do with it.
    /// // Let’s fall back to the default:
    /// let status = status.class().default_code();
    ///
    /// // And look! That is 400 Bad Request.
    /// assert_eq!(status, BadRequest);
    /// // So now let’s treat it as that.
    /// ```
    #[inline]
    pub fn default_code(&self) -> StatusCode {
        unsafe { transmute::<StatusClass, StatusCode>(*self) }
    }
}

impl ToPrimitive for StatusClass {
    fn to_i64(&self) -> Option<i64> {
        Some(*self as i64)
    }

    fn to_u64(&self) -> Option<u64> {
        Some(*self as u64)
    }
}
