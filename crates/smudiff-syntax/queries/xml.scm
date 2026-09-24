(Comment) @comment
(AttValue) @string
[(EntityRef) (CharRef)] @escape

[(STag (Name) @tag) (ETag (Name) @tag) (EmptyElemTag (Name) @tag)]
(Attribute (Name) @attribute)

["xml" "DOCTYPE"] @keyword
["version" "encoding" "standalone"] @attribute
(XMLDecl ["\"" "'" (VersionNum) (EncName) "yes" "no"] @string)
