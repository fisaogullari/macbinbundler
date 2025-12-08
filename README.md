# About

Macbinbundler is a simple cli tool copies of an executable or a dynamic library to a given folder as well as all its dependant libraries. As such a portable version of an executable or a dynamic library could be produced. This is particulary helpful if one willing to use an executable as a subprocess or put a dynamic library to a mac app bundle.

# Installation

Macbinbundler can be installed with `Homebrew`:

```
$ brew tap fisaogullari/homebrew-macbinbundler
$ brew install --HEAD macbinbundler
```

If you desire to build yourself, you can simply run:

```
$ git clone https://github.com/fisaogullari/macbinbundler.git
$ cd macbinbundler
$ cargo build --release
```

or (optional for installing to cargo path):

```
$ cargo install --path .
```

# Uninstallation

You should run following commands:

```
$ brew uninstall macbinbundler
$ brew untap fisaogullari/homebrew-macbinbundler
```

# Usage

Macbinbundler is quite simple to use. You can run following command to print out usage info:

```
$ macbinbundler -h
```

# Examples

For simple use cases, usage is straightforward.
For instance following command will copy the executable `pdftoppm` to `~/Projects/foo` folder and all dependant libraries inside of the `~/Projects/foo/libs` folder.

```
$ macbinbundler -i /opt/homebrew/bin/pdftoppm -o ~/Projects/foo
```

Also custom dependency folder can be given like so (Note: Dependency folder must be relevant to destination folder!):

```
$ macbinbundler -i /opt/homebrew/bin/pdftoppm -o ~/Projects/foo/bar -d ../Frameworks
```

# Contact

If you want to contact me, you can create an issue or simply send an email to `fisaogullari@gmail.com`.
