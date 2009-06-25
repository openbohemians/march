#!/usr/bin/env ruby

#require 'calibre/overload'


module Fly


  class Word

    #def initialize( name, face, blk=nil, &block )
    #  @name = name.to_s
    #  @face = parse_interface( face )
    #  @code = blk || block
    #end

    attr :name
    attr :code

    def initialize( code, lang='fly' )
      #@code = str
      @lang = lang
      @face = nil
      @code = code
      #@local_stack = []
    end

  end


end
