header! {
    /// `Last-Event-ID` header, defined in
    /// [RFC3864](https://html.spec.whatwg.org/multipage/references.html#refsRFC3864)
    ///
    /// The `Last-Event-ID` header contains information about
    /// the last event in an http interaction so that it's easier to
    /// track of event state. This is helpful when working
    /// with [Server-Sent-Events](http://www.html5rocks.com/en/tutorials/eventsource/basics/). If the connection were to be dropped, for example, it'd
    /// be useful to let the server know what the last event you
    /// recieved was.
    ///
    /// The spec is a String with the id of the last event, it can be
    /// an empty string which acts a sort of "reset".
    ///
    /// # Example
    /// ```
    /// use hyper::header::{Headers, LastEventID};
    ///
    /// let mut headers = Headers::new();
    /// headers.set(LastEventID("1".to_owned()));
    /// ```
    (LastEventID, "Last-Event-ID") => [String]

    test_last_event_id {
        // Initial state
        test_header!(test1, vec![b""]);
        // Own testcase
        test_header!(test2, vec![b"1"], Some(LastEventID("1".to_owned())));
    }
}
