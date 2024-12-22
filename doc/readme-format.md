Readme Format
=============

`README.md` is a Markdown file containing the following elements, in order:

- One or more image hyperlinks serving as *badges*, one per line, in the form
  `[![Alt text](image-url)](link-url)`.

    - Badges with the following image URLs are recognized and treated specially
      by `rsrepo`:

        - `https://www.repostatus.org/badges/latest/{status}.svg` — A
          [repostatus.org](https://www.repostatus.org) badge

        - `https://img.shields.io/badge/MSRV-{version}-{color}` — An MSRV badge

- A blank line

- Optional:

    - A line consisting of one or more *header links*, each one of which is a
      Markdown link of the form `[Text](url)`.  Adjacent links are separated by
      whitespace, a vertical bar (`|`), and whitespace.

        - Header links with the following text are recognized and treated
          specially by `rsrepo`:

            - "GitHub"
            - "crates.io"
            - "Documentation"
            - "Changelog"

    - A blank line

- Arbitrary freeform text
