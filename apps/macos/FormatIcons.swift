import AppKit

@MainActor
struct FormatIcon {
    let color: NSColor
    let image: NSImage?

    init(for id: String) {
        switch id {
        case "rich-text": self.init(0x6546B7, symbol: "textformat")
        case "plain-text": self.init(0x505D71, symbol: "text.alignleft")
        case "html-source": self.init(0x167473, symbol: "chevron.left.forwardslash.chevron.right")
        case "slack-paste", "slack-api": self.init(0x3C193D, artwork: Self.slack)
        case "discord": self.init(0x4B55DE, artwork: Self.discord)
        case "telegram": self.init(0x037CB2, artwork: Self.telegram)
        case "signal": self.init(0x3A76F0, symbol: "bubble.left.and.bubble.right.fill")
        case "teams": self.init(0x5B5FC7, symbol: "person.2.fill")
        case "word": self.init(0x185ABD, symbol: "doc.text.fill")
        case "whatsapp": self.init(0x167A4A, symbol: "phone.bubble.fill")
        case "reddit": self.init(0xCC3900, artwork: Self.reddit)
        case "facebook": self.init(0x1861CF, artwork: Self.facebook)
        case "twitter": self.init(0x111318, artwork: Self.x)
        case "sms": self.init(0x287948, symbol: "message.fill")
        default: self.init(0x505D71, symbol: "doc")
        }
    }

    private init(_ rgb: UInt32, symbol: String) {
        color = Self.color(rgb)
        let configuration = NSImage.SymbolConfiguration(pointSize: 18, weight: .semibold)
            .applying(.init(paletteColors: [.white]))
        image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)?.withSymbolConfiguration(configuration)
    }

    private init(_ rgb: UInt32, artwork: String) {
        color = Self.color(rgb)
        image = Data(base64Encoded: artwork, options: .ignoreUnknownCharacters).flatMap(NSImage.init(data:))
    }

    private static func color(_ rgb: UInt32) -> NSColor {
        NSColor(srgbRed: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255,
                blue: CGFloat(rgb & 255) / 255, alpha: 1)
    }

    // Artwork from https://github.com/simple-icons/simple-icons/tree/13.21.0/icons.
    // Embedded at 3x so the standalone pasteboard helper can show the same marks.
    private static let slack = """
    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
    k6Hy26MTLPsAAAAASUVORK5CYII=
    """

    private static let discord = """
    iVBORw0KGgoAAAANSUhEUgAAAEIAAABCCAYAAADjVADoAAAABmJLR0QA/wD/AP+gvaeTAAAE80lEQVR4nO2bW4hVZRiGn3+rmDpq
    WZiaaUZhYnbAsZOmmRFhBRZUmGSSBCVEh4u6aDp5oVdGUXobREV5FRRhWHQiDbMgCrtQNMlzHkcdFR2fLtaeXLPbp7XXmtkz435g
    wey1v/1+7/73tw7fv/6BBg0aNGjQoEFPQr24J2oVI2QlpA4CZgKzgOb8NgCYEELYn1J7JLANOAlszG/fAT+EEE6k0c4EdYz6rPql
    esLiLMsgz/IS2m3qGvUZdXSaHIkrQm0CHgXmE1VArsJHjhJVxYGYxnBgINAEXAAInMrHngohtMZiRwB/AUMr5GknqpKPgI9DCMer
    /1YJBkKdCCwBFgHDkiQB1gFtwFhgPDCoQnwbsB3YmY+dnjDfEeA9YFUIYXM1H6hqINTpwPdU/vV7Gu3AjBDCT5UCKw6E2p/o5HR9
    Bsbqwe/A1BDC6XJB1fzCz9F7BwFgCvB8paCyFaGOBf4kOqn1ZtqAKSGEraUCKlXEW/T+QQAYDKwoF1CyItRbgPVZO6ozt5Y6cZar
    iNe6yEw9WVrqjaIVoTYDG0q938u5M4TwTeHOUhWxlL45CFCiKv73ZdVpRNXQl2kOIfwS31GsIp7uJjP1ZEnhjk4VoQ4FdtE3Lpnl
    OAFcHm8ECytiAX1/ECBq5BbFdxQOxOJus1J/Ho+/+O/QUCcDf3S7nfpydQhhC3SuiPvqZKaePNjxR3wg5maYYC+wm2jmKSsE9uS3
    rHigcwa9UD1dYl6wWk6qy/Ida4fuSPUltTWF7jG1RR0V0x2jvm40Z5mGduNznerDKQWPqDNK/pQ6Sd1Zg+4+9YYyus3q/pTeH4oL
    rkwp9kilGlRvU88k0Dyr3l2F7tyU3lfExdanEKo4HxjL80kC3S8S6K5N4f9HgJzaD7iu2qRF+CBB7Ic9QLeQqerAHDCJaAanVn7u
    ZbGFDAQm54AbU4gAHEwQezhB7KEuii3GlTlgQkqREV0Ue1EX6RZjQg64LKXItASxN3eR7k0JYosxPgeMSSnyWILYBT1AtxgTUH9N
    cenpYH6lTOrtRndySbinCt17M/D/G+qODIRa1ZllzE5Wd9Wg+486tYxuFneWqpvJJ8uCU0brGOK9xiXqi6brNY6rr9i51xilvmr6
    XqODv4N6GBheqQQT0NElngVGAf0y0j3Luc5zFNk+mT8Q1DYqr1fo67TlgP71dtEDGJAjWkxxvnMsR7TM5nynNQe0Vgzr+xzNkawR
    KuQ08C31PbzOZODhaI7oyVatzA8hzAauIFpGsD2FVlK2AS3AuLyHRSm0dqMOUd8xmhpLygGjydnBAGpOvUt9U92Uzb1OJzblteeo
    uXzOJvVl9VANeu3q2+b9kxecrW6t0eBuo7u/S+PDrI5TnzAa6K/ycdWyR/1afVddrI4v0B6tvqHurdHzFmNtQeFD4CZgOfAUtd1f
    nALmhRDWlAowWkk7kmjR6jDOzTscIlp52wrsiz+gLaIxD1hNtNY7KaeBlUBLxdW56kR1dQ2jvEMdUoO5RBgdDrU0cWvVa2tJOEvd
    kCDRwi743qW8PZnA1zqj1cOpk85RP7f8fMJ68yew7kDtp24s46dd/VS9oyuST1RXqYcLkrap12SesLKfKUbtf5yDRifnq7rDwED1
    fvV9o2eTL3R50tJeWoz+V+QzdaHxS2E3G2nqzkOiSP6c0dWuQYMGDRo0aNC9/AvX6BGSUMCuqgAAAABJRU5ErkJggg==
    """

    private static let telegram = """
    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
    TkSuQmCC
    """

    private static let reddit = """
    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
    """

    private static let facebook = """
    iVBORw0KGgoAAAANSUhEUgAAAEIAAABCCAYAAADjVADoAAAABmJLR0QA/wD/AP+gvaeTAAAFN0lEQVR4nOWbya8UVRSHv1OgiAiC
    QEQhUYMyCCjRxIBBYiAOURMX4LAyutAocYiaqAsT/RfculCjuDEOATUoEjcaBgeQqI+AMiSCaEQBmWTyc1Hd0DSv3+vqV7e7X/yS
    Tqe7b597zq9u3bp17qmgDajnAdcB04EpwFTgQmAkMLrSbB9woPK+pfLqAdZHxNHUPkYqw+pUYDGwAJgLDG/R1BFgNfA58G5EbCnH
    w4SoF6hL1LWmY436mHpBp+M9C3WU+rz6Z0IB6tmjvqxe1On4UYeqT6v72yhAPfvUJ9UhnRJhjrqxgwLUs0G9oZ0ChPqUeqyzcffK
    cfPTJUstwhh1RWdjbYqP1NH9R3Sapi+f6gTgE+DaogJ2iB+B2yNiZzONmxJCvQpYCVzeul8dYTtwa0T83F/DfoVQJwJfMvhEqLIT
    uDEifumrUZ+Tinoh8DGDVwSAScAK+1lvNBwR5tflleRL5HaxG9gIbCa/56i+TgAX1bwuBWYBk4Fm1w+fkc8Z/xbyyPwy1A52qC+p
    Mws5mPs4TF1ZoK8Xi3Zws3qi/JjP4C/1UfWcogLU+fpBgT5PqDc1a3iYujlB4LV8ZT4JDxiLCaHao55bb6e3yfI58pxBKlYDCyJi
    V8I++mI68EyfLdRJ6uGyD38NO9VxZUZl8RGhelC9pNZO/Yh4ltYTKM2wJCL2JLTfLCNoNCrUseqBMg9/HWtSRGRrI0LzUTG+aqd2
    RCwBUmZ9XklouxVGAI9UP2SQ31oDDyTs9AiwLKH9VjkV89DK+1zgyoQdro+II638UZ0L3EV+JRtHnvmush8ovBCrYYo6JyLWVoW4
    bwDGmmFd0T+oY4E3yEVIyb3AKSEWJu7sqyKNze9z3gfmp3HnDBYCZOrFwNWJO+vzFrgXFtEeEQBmqeMzYB4JN3oq7CvYfnESL3on
    gHkZ6UcDFBdiWhIvGjM9I9+HTM3+gu3H99+kVKZlpL3BqnKiYPt2CzEl60CnzdDuXatxGWmX1YOFkRlnrtT+r4waSnnDcDGwocFv
    xwramtxku4XAqwVt98aQocBhYFQJxn6NiG0l2KFZO+r9ZfQHHMzIhRislLX9eDADfi/JWCe4piQ7f2bAjpKMtRX1fOCqksz9nAGl
    nNcdYCblTfRbMxrP9N1OmeUJPRktJE26hDKF+CIDfgL+KNFouyhrotwVEduziJB863/QUEk2lyXEKjidvF0GPDhAg6+phxr8dmNE
    NL26VL/pp8kQ8hLmMngHKpkpdTiwCxhTkvF6zitST62ayI969gITIuJYBlBJtS9tU+fdxNLqSD2Vq1RnAN+TJn/ZjSPiJDA1IrZC
    zZZfRPxId+5GpWJ5VQSoO/rqbGB9/fcl0G0j4iQwOyJ+qH5xRllARHwHvJ3YiW7g9VoRoJcjb15hu4nTT9aUQTeNiL3AjIjYXfvl
    WaVDEfEb8EJCRzrNE/Ui9In6XosFGL0xrIinJfZbz3uN+uyr4HQM8C1wRZEgGtANp8ZmYE5E9Lrr1rAEOSL2AneSn1ODnQPAokYi
    QD+12BGxCbgHSP64YUIOA3dX1kkDQ73NgZUddmqOOKreMWAB6py7Rf27RYc6IcReNU1BvTpT3TYIhNimTk8iQo2T49WPu1iIZZZc
    3duXo6E+rh7qIiEOqo+lirk/hyeqb3aBEB+ql6WKs4jj89VVHRDiU5t9/qKdqHPVtzz7UlumEIfUper1qeIoDXW0+rC6vOL4QIU4
    YP5Q60NqGTv3Z5G6rLCaGP6nsm3Q7H++Jk8b9gBrgXURcTyRiwD8B9Efu5LGHwJdAAAAAElFTkSuQmCC
    """

    private static let x = """
    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
    """
}
